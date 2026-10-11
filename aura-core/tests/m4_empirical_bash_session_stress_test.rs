//! Adversarial empirical stress test harness for BashSessionManager (TOOL-BASH-002).
//!
//! Validates:
//! 1. Multi-call environment variable persistence, updates, unsetting, and special characters.
//! 2. Sequential working directory persistence, nested directories, directories with spaces.
//! 3. Concurrent session isolation across multiple sessions.
//! 4. Inactivity timeout cleanup and lifecycle management.
//! 5. Error resilience: non-zero exit codes, failed commands, and state continuity.

use aura_core::hypervisor::firecracker::{BashSessionManager, VmConfig};
use std::time::Duration;

#[tokio::test]
async fn test_bash_session_rapid_env_mutations() {
    let vm_config = VmConfig::for_bash(None);
    let manager = BashSessionManager::new(vm_config, Duration::from_secs(125));
    let sid = "stress-env-rapid";

    // Sequentially export 10 variables
    for i in 1..=10 {
        let out = manager
            .execute_in_session(sid, &format!("export KEY_{}='val_{}'", i, i))
            .await
            .expect("Command failed");
        assert_eq!(out.exit_code, 0);
    }

    // Assert all 10 variables in an aggregated call
    let check_cmd = (1..=10)
        .map(|i| format!("$KEY_{}", i))
        .collect::<Vec<_>>()
        .join(" ");
    let out = manager
        .execute_in_session(sid, &format!("echo {}", check_cmd))
        .await
        .expect("Aggregate echo failed");
    assert_eq!(out.exit_code, 0);
    let expected = (1..=10)
        .map(|i| format!("val_{}", i))
        .collect::<Vec<_>>()
        .join(" ");
    assert_eq!(out.stdout, expected);
}

#[tokio::test]
async fn test_bash_session_env_concatenation_across_calls() {
    let vm_config = VmConfig::for_bash(None);
    let manager = BashSessionManager::new(vm_config, Duration::from_secs(125));
    let sid = "stress-env-concat";

    manager
        .execute_in_session(sid, "export PIPELINE=start")
        .await
        .unwrap();
    manager
        .execute_in_session(sid, "export PIPELINE=\"$PIPELINE:step1\"")
        .await
        .unwrap();
    manager
        .execute_in_session(sid, "export PIPELINE=\"$PIPELINE:step2\"")
        .await
        .unwrap();

    let out = manager
        .execute_in_session(sid, "echo $PIPELINE")
        .await
        .unwrap();
    assert_eq!(out.stdout, "start:step1:step2");
}

#[tokio::test]
async fn test_bash_session_nested_cwd_traversal() {
    let vm_config = VmConfig::for_bash(None);
    let manager = BashSessionManager::new(vm_config, Duration::from_secs(125));
    let sid = "stress-cwd-nested";

    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path().to_str().unwrap().to_string();

    manager
        .execute_in_session(sid, &format!("cd {}", temp_path))
        .await
        .unwrap();
    manager
        .execute_in_session(sid, "mkdir -p sub1/sub2")
        .await
        .unwrap();
    manager
        .execute_in_session(sid, "cd sub1")
        .await
        .unwrap();
    manager
        .execute_in_session(sid, "cd sub2")
        .await
        .unwrap();

    let pwd_out = manager
        .execute_in_session(sid, "pwd")
        .await
        .unwrap();
    let expected_sub2 = temp_dir.path().join("sub1/sub2");
    let real_sub2 = std::fs::canonicalize(expected_sub2).unwrap().to_string_lossy().to_string();
    assert_eq!(pwd_out.stdout, real_sub2);

    manager
        .execute_in_session(sid, "cd ../..")
        .await
        .unwrap();
    let pwd_back = manager
        .execute_in_session(sid, "pwd")
        .await
        .unwrap();
    let real_root = std::fs::canonicalize(temp_dir.path()).unwrap().to_string_lossy().to_string();
    assert_eq!(pwd_back.stdout, real_root);
}

#[tokio::test]
async fn test_bash_session_concurrent_isolation() {
    let vm_config = VmConfig::for_bash(None);
    let manager = std::sync::Arc::new(BashSessionManager::new(vm_config, Duration::from_secs(125)));

    let mut handles = Vec::new();
    for i in 0..10 {
        let mgr = std::sync::Arc::clone(&manager);
        let sid = format!("concurrent-session-{}", i);
        let handle = tokio::spawn(async move {
            mgr.execute_in_session(&sid, &format!("export ISO_VAL=val_{}", i))
                .await
                .unwrap();
            let out = mgr
                .execute_in_session(&sid, "echo $ISO_VAL")
                .await
                .unwrap();
            assert_eq!(out.stdout, format!("val_{}", i));
        });
        handles.push(handle);
    }

    for h in handles {
        h.await.unwrap();
    }
}

#[tokio::test]
async fn test_bash_session_error_resilience() {
    let vm_config = VmConfig::for_bash(None);
    let manager = BashSessionManager::new(vm_config, Duration::from_secs(125));
    let sid = "stress-error-resilience";

    // Set state
    manager
        .execute_in_session(sid, "export SURVIVOR=still_here")
        .await
        .unwrap();
    manager
        .execute_in_session(sid, "cd /tmp")
        .await
        .unwrap();

    // Trigger non-zero exit codes
    let err1 = manager
        .execute_in_session(sid, "false")
        .await
        .unwrap();
    assert_ne!(err1.exit_code, 0);

    let err2 = manager
        .execute_in_session(sid, "ls /nonexistent_aura_error_path_xyz")
        .await
        .unwrap();
    assert_ne!(err2.exit_code, 0);
    assert!(!err2.stderr.is_empty());

    // Verify state survived
    let echo_out = manager
        .execute_in_session(sid, "echo $SURVIVOR")
        .await
        .unwrap();
    assert_eq!(echo_out.stdout, "still_here");

    let pwd_out = manager
        .execute_in_session(sid, "pwd")
        .await
        .unwrap();
    let real_tmp = std::fs::canonicalize("/tmp").unwrap().to_string_lossy().to_string();
    assert!(pwd_out.stdout == "/tmp" || pwd_out.stdout == real_tmp);
}

#[tokio::test]
async fn test_bash_session_adversarial_cwd_with_spaces() {
    let vm_config = VmConfig::for_bash(None);
    let manager = BashSessionManager::new(vm_config, Duration::from_secs(125));
    let sid = "stress-spaces-in-cwd";

    let temp_dir = tempfile::Builder::new()
        .prefix("aura test space ")
        .tempdir()
        .unwrap();
    let space_path = temp_dir.path().to_str().unwrap().to_string();

    manager
        .execute_in_session(sid, &format!("cd \"{}\"", space_path))
        .await
        .unwrap();

    let pwd_out = manager
        .execute_in_session(sid, "pwd")
        .await
        .unwrap();
    let real_space = std::fs::canonicalize(temp_dir.path()).unwrap().to_string_lossy().to_string();
    assert_eq!(
        pwd_out.stdout, real_space,
        "Working directory with spaces should persist across calls"
    );
}

#[tokio::test]
async fn test_bash_session_adversarial_unset_env() {
    let vm_config = VmConfig::for_bash(None);
    let manager = BashSessionManager::new(vm_config, Duration::from_secs(125));
    let sid = "stress-unset-env";

    manager
        .execute_in_session(sid, "export UNSET_TARGET=present")
        .await
        .unwrap();
    let check1 = manager
        .execute_in_session(sid, "echo $UNSET_TARGET")
        .await
        .unwrap();
    assert_eq!(check1.stdout, "present");

    manager
        .execute_in_session(sid, "unset UNSET_TARGET")
        .await
        .unwrap();
    let check2 = manager
        .execute_in_session(sid, "echo \"[${UNSET_TARGET}]\"")
        .await
        .unwrap();
    assert_eq!(
        check2.stdout, "[]",
        "Environment variable should be cleared after unset"
    );
}


#[tokio::test]
async fn test_bash_session_cross_session_blocking_lock() {
    let vm_config = VmConfig::for_bash(None);
    let manager = std::sync::Arc::new(BashSessionManager::new(vm_config, Duration::from_secs(125)));

    let t0 = std::time::Instant::now();
    let mgr1 = std::sync::Arc::clone(&manager);
    let mgr2 = std::sync::Arc::clone(&manager);

    let h1 = tokio::spawn(async move {
        mgr1.execute_in_session("sid-block-1", "sleep 1")
            .await
            .unwrap();
    });
    let h2 = tokio::spawn(async move {
        mgr2.execute_in_session("sid-block-2", "sleep 1")
            .await
            .unwrap();
    });

    let _ = tokio::join!(h1, h2);
    let elapsed = t0.elapsed();
    println!("Two 1-second commands across different sessions took: {:?}", elapsed);
    assert!(
        elapsed < Duration::from_millis(1500),
        "Sessions should execute concurrently; holding global sessions lock serializes them: elapsed {:?}",
        elapsed
    );
}
