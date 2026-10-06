use aura_core::hypervisor::wasm_tools::WasmTools;

#[test]
fn test_wasm_fs_write_read() {
    let temp_file = tempfile::Builder::new()
        .tempfile_in(std::env::current_dir().unwrap())
        .expect("Failed to create temp file");
    let path = temp_file.path().to_str().expect("Invalid path");
    let content = "Hello WebAssembly";
    
    // Write
    let write_res = WasmTools::fs_write(path, content);
    assert!(write_res.is_ok(), "Failed to write file via WASM tools: {:?}", write_res.err());

    // Read
    let read_res = WasmTools::fs_read(path);
    assert!(read_res.is_ok(), "Failed to read file via WASM tools");
    assert_eq!(read_res.unwrap(), content);
}
