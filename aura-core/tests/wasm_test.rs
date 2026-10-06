use aura_core::hypervisor::wasm_tools::WasmTools;
use tempfile::tempdir;
use std::env;
use serial_test::serial;

#[test]
#[serial]
fn test_wasm_fs_write_read() {
    let dir = tempdir().unwrap();
    let base_path = dir.path().to_str().unwrap().to_string();
    env::set_var("AURA_BASE_DIR", &base_path);

    let path = format!("{}/test_wasm_fs.txt", base_path);
    let content = "Hello WebAssembly";
    
    // Write
    let write_res = WasmTools::fs_write(&path, content);
    assert!(write_res.is_ok(), "Failed to write file via WASM tools");

    // Read
    let read_res = WasmTools::fs_read(&path);
    assert!(read_res.is_ok(), "Failed to read file via WASM tools");
    assert_eq!(read_res.unwrap(), content);

    // Cleanup handled by tempdir drop
}
