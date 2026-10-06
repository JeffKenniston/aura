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
fn test_wasm_fs_read_missing_file() {
    let path = "/home/jeff/aura/tests/this_file_does_not_exist.txt";

    let read_res = WasmTools::fs_read(path);
    assert!(read_res.is_err(), "Expected error when reading a missing file");
}

#[test]
fn test_wasm_fs_read_path_traversal_restricted_dir() {
    let path = "/etc/passwd";

    let read_res = WasmTools::fs_read(path);
    assert!(read_res.is_err(), "Expected error when path is outside restricted directory");
    assert_eq!(read_res.unwrap_err().to_string(), "Path traversal blocked: Access restricted to /home/jeff/aura");
}

#[test]
fn test_wasm_fs_read_path_traversal_dot_dot() {
    let path = "/home/jeff/aura/../aura/tests/test_wasm_fs.txt";

    let read_res = WasmTools::fs_read(path);
    assert!(read_res.is_err(), "Expected error when path contains ..");
    assert_eq!(read_res.unwrap_err().to_string(), "Path traversal blocked: .. is not allowed");
}
