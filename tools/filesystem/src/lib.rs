pub mod patch;
pub mod path;
pub mod service;

pub use patch::apply_patch;
pub use path::{FileSystemError, PathValidator};
pub use service::FileSystemService;

// Generate WIT bindings for the aura:fs WebAssembly component
wit_bindgen::generate!({
    path: "wit",
    world: "filesystem",
});

struct FileSystemComponent;

impl exports::aura::fs::read::Guest for FileSystemComponent {
    fn read_file(path: String) -> Result<Vec<u8>, String> {
        let service = FileSystemService::default();
        service.read_file(&path)
    }
}

impl exports::aura::fs::write::Guest for FileSystemComponent {
    fn write_file(path: String, contents: Vec<u8>) -> Result<(), String> {
        let service = FileSystemService::default();
        service.write_file(&path, &contents)
    }

    fn patch_file(path: String, diff: String) -> Result<String, String> {
        let service = FileSystemService::default();
        service.patch_file(&path, &diff)
    }
}

#[cfg(target_arch = "wasm32")]
export!(FileSystemComponent);
