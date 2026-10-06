use aura_core::hypervisor::wasm_tools::WasmTools;
use std::env;

#[test]
fn test_wasm_fs_write_read() {
    let temp_dir = env::temp_dir();
    let file_path = temp_dir.join("test_wasm_fs.txt");
    let path = file_path.to_str().unwrap();
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
