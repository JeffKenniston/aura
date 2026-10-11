use aura_core::hypervisor::firecracker::VmConfig;
use aura_core::knowledge::git::{GitManager, CONFLICT_DETECTED};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

/// Helper to create a temporary directory with `count` files initialized to uniform content.
fn setup_test_files(count: usize, size: usize) -> (tempfile::TempDir, Vec<PathBuf>) {
    let dir = tempfile::tempdir().expect("Failed to create tempdir");
    let mut files = Vec::with_capacity(count);

    for i in 0..count {
        let file_path = dir.path().join(format!("file_{:03}.rs", i));
        let content = vec![b'a' + (i % 26) as u8; size];
        fs::write(&file_path, &content).expect("Failed to write initial file");
        files.push(file_path);
    }

    (dir, files)
}

#[test]
fn test_stress_identical_file_length_drift_detection() {
    // Stress test: mutating bytes without changing file length must trigger hash-level drift detection
    const FILE_COUNT: usize = 8;
    const FILE_SIZE: usize = 512;
    let (_dir, files) = setup_test_files(FILE_COUNT, FILE_SIZE);
    let git = GitManager::new("main").with_files(&files);

    // Initial state: clean drift
    assert_eq!(git.check_drift(), Ok(()));

    for (idx, file) in files.iter().enumerate() {
        // Read current bytes
        let mut bytes = fs::read(file).expect("Failed to read file");
        assert_eq!(bytes.len(), FILE_SIZE);

        // Mutate a single byte in the middle of the file (length remains identical)
        bytes[FILE_SIZE / 2] ^= 0xFF;
        fs::write(file, &bytes).expect("Failed to write mutated file");
        assert_eq!(fs::metadata(file).unwrap().len() as usize, FILE_SIZE);

        // Drift check MUST detect conflict because BLAKE3 hash changed
        let drift_res = git.check_drift();
        assert_eq!(
            drift_res,
            Err(CONFLICT_DETECTED),
            "Failed to detect identical-length byte mutation on file {}",
            idx
        );

        // Revert byte back to original
        bytes[FILE_SIZE / 2] ^= 0xFF;
        fs::write(file, &bytes).expect("Failed to restore original file");

        // Verify drift clears
        assert_eq!(
            git.check_drift(),
            Ok(()),
            "Drift should clear after reverting file {}",
            idx
        );
    }
}

#[test]
fn test_stress_multiple_external_modifications() {
    const FILE_COUNT: usize = 20;
    let (_dir, files) = setup_test_files(FILE_COUNT, 128);
    let git = GitManager::new("main").with_files(&files);

    assert_eq!(git.check_drift(), Ok(()));

    // Concurrently mutate 5 distinct files externally without agent authorship
    let external_indices = vec![2, 7, 11, 15, 19];
    for &idx in &external_indices {
        fs::write(&files[idx], b"external modification payload").unwrap();
    }

    // Drift must fail
    assert_eq!(git.check_drift(), Err(CONFLICT_DETECTED));

    // Register 4 of them as agent-authored; 1 remains unauthored
    for &idx in &external_indices[0..4] {
        git.record_agent_change(&files[idx]);
    }

    // Still must fail because files[19] is unauthored
    assert_eq!(git.check_drift(), Err(CONFLICT_DETECTED));

    // Register the final one
    git.record_agent_change(&files[19]);

    // Now all modified files are authored, check_drift should succeed
    assert_eq!(git.check_drift(), Ok(()));
}

#[test]
fn test_stress_interleaved_agent_authoring_and_external_edits() {
    let (_dir, files) = setup_test_files(6, 64);
    let git = GitManager::new("main").with_files(&files);

    // Agent authors file 0 and file 2
    git.record_agent_change(&files[0]);
    fs::write(&files[0], b"agent edit to file 0").unwrap();

    git.record_agent_change(&files[2]);
    fs::write(&files[2], b"agent edit to file 2").unwrap();

    // Check drift before any external edit -> OK
    assert_eq!(git.check_drift(), Ok(()));

    // External actor edits file 4
    fs::write(&files[4], b"external intruder edit to file 4").unwrap();

    // Check drift -> Conflict
    assert_eq!(git.check_drift(), Err(CONFLICT_DETECTED));

    // Agent authors file 5 while file 4 is still conflicting
    git.record_agent_change(&files[5]);
    fs::write(&files[5], b"agent edit to file 5").unwrap();

    // Still conflict because file 4 is dirty and unauthored
    assert_eq!(git.check_drift(), Err(CONFLICT_DETECTED));

    // External actor's edit to file 4 is reverted to original content
    let original_content = vec![b'a' + 4; 64];
    fs::write(&files[4], &original_content).unwrap();

    // After reverting unauthored file, drift check succeeds (authored file 5 is recognized)
    assert_eq!(git.check_drift(), Ok(()));
}

#[test]
fn test_stress_apply_patch_halting_conditions() {
    let (_dir, files) = setup_test_files(4, 128);
    let git = GitManager::new("main").with_files(&files);

    // 1. Halts immediately on unauthored drift
    fs::write(&files[1], b"unauthored dirty change").unwrap();
    let res = git.apply_patch("diff 1");
    assert_eq!(res, Err(CONFLICT_DETECTED.to_string()));
    assert!(git.committed_patches().is_empty());

    // Fix drift: register authorship
    git.record_agent_change(&files[1]);

    // 2. Halts immediately on explicit linter error
    git.set_linter_error("ruff: unexpected indentation at line 12");
    let res = git.apply_patch("diff 1");
    assert_eq!(res, Err("ruff: unexpected indentation at line 12".to_string()));
    assert!(git.committed_patches().is_empty());

    // Clear explicit linter error
    if let Ok(mut l) = git.linter_error.lock() {
        *l = None;
    }

    // 3. Halts on invalid microVM configuration: vcpu_count == 0
    git.set_vm_config(VmConfig {
        vcpu_count: 0,
        ..VmConfig::for_bash(None)
    });
    let res = git.apply_patch("diff 1");
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("vcpu_count"));
    assert!(git.committed_patches().is_empty());

    // 4. Halts on invalid microVM configuration: mem_size_mib < 16
    git.set_vm_config(VmConfig {
        mem_size_mib: 8,
        ..VmConfig::for_bash(None)
    });
    let res = git.apply_patch("diff 1");
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("mem_size_mib"));
    assert!(git.committed_patches().is_empty());

    // 5. Halts on invalid microVM configuration: empty rootfs_path
    git.set_vm_config(VmConfig {
        rootfs_path: "".to_string(),
        ..VmConfig::for_bash(None)
    });
    let res = git.apply_patch("diff 1");
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("rootfs_path"));
    assert!(git.committed_patches().is_empty());

    // 6. Halts on invalid microVM configuration: empty kernel_path
    git.set_vm_config(VmConfig {
        kernel_path: "   ".to_string(),
        ..VmConfig::for_bash(None)
    });
    let res = git.apply_patch("diff 1");
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("kernel_path"));
    assert!(git.committed_patches().is_empty());

    // 7. Successful application when all gates pass
    git.set_vm_config(VmConfig::for_bash(None));
    let res = git.apply_patch("feat: initial patch");
    assert!(res.is_ok());
    assert_eq!(git.committed_patches(), vec!["feat: initial patch"]);

    // 8. Subsequent patch fails if a new unauthored modification appears
    fs::write(&files[2], b"another external change").unwrap();
    let res2 = git.apply_patch("feat: second patch");
    assert_eq!(res2, Err(CONFLICT_DETECTED.to_string()));
    // History must still only contain the first patch
    assert_eq!(git.committed_patches(), vec!["feat: initial patch"]);
}

#[test]
fn test_stress_multithreaded_concurrent_git_operations() {
    let (_dir, files) = setup_test_files(10, 64);
    let git = Arc::new(GitManager::new("main").with_files(&files));

    let thread_count = 12;
    let iterations_per_thread = 50;
    let mut handles = Vec::with_capacity(thread_count);

    for t in 0..thread_count {
        let git_clone = Arc::clone(&git);
        let files_clone = files.clone();

        let handle = thread::spawn(move || {
            for i in 0..iterations_per_thread {
                match (t + i) % 4 {
                    0 => {
                        // Check drift
                        let _ = git_clone.check_drift();
                    }
                    1 => {
                        // Check authorship
                        let target = &files_clone[i % files_clone.len()];
                        let _ = git_clone.is_agent_authored(target);
                    }
                    2 => {
                        // Open PR
                        let title = format!("thread-{}-pr-{}", t, i);
                        let res = git_clone.open_pr(&title, "Automated concurrent PR");
                        assert!(res.is_ok(), "PR creation should succeed");
                    }
                    3 => {
                        // Record agent change
                        let target = &files_clone[(t + i) % files_clone.len()];
                        git_clone.record_agent_change(target);
                    }
                    _ => unreachable!(),
                }
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("Thread panicked during concurrent stress test");
    }

    // Verify post-concurrency invariant:
    // No active ephemeral tokens should remain leaked in active_tokens
    let active_count = git.active_tokens.lock().unwrap().len();
    assert_eq!(
        active_count, 0,
        "All ephemeral tokens must be revoked after multithreaded operations"
    );

    let revoked_count = git.revoked_tokens.lock().unwrap().len();
    assert!(
        revoked_count > 0,
        "Revoked tokens should have recorded PR credentials"
    );
}
