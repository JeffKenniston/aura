use aura_core::hypervisor::wasm_tools::WasmTools;

#[test]
fn test_wasm_fs_write_read() {
    let path = "/home/jeff/aura/tests/test_wasm_fs.txt";
    let content = "Hello WebAssembly";
    
    // Write
    let write_res = WasmTools::fs_write(path, content);
    assert!(write_res.is_ok(), "Failed to write file via WASM tools");

    // Read
    let read_res = WasmTools::fs_read(path);
    assert!(read_res.is_ok(), "Failed to read file via WASM tools");
    assert_eq!(read_res.unwrap(), content);

    // Cleanup
    let _ = std::fs::remove_file(path);
}

#[test]
fn test_wasm_fs_broken_symlink_traversal_blocked() {
    let target_path = "/tmp/secret_file_broken.txt";
    // Create a symlink pointing to a file that doesn't exist yet
    let symlink_path = "/home/jeff/aura/tests/test_symlink_broken.txt";
    let _ = std::fs::remove_file(symlink_path);
    std::os::unix::fs::symlink(target_path, symlink_path).unwrap();

    // Attempt to write via the broken symlink
    let write_res = aura_core::hypervisor::wasm_tools::WasmTools::fs_write(symlink_path, "new content");

    // Writing should either fail because it's blocked, or fail because canonicalize fails on broken symlink.
    // In any case, it should NOT succeed and create the file outside.
    assert!(write_res.is_err(), "Broken symlink traversal should be blocked");
    assert!(!std::path::Path::new(target_path).exists(), "File should not be created outside");

    // Cleanup
    let _ = std::fs::remove_file(symlink_path);
    let _ = std::fs::remove_file(target_path);
}
