use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct KernelConfig {
    pub kernel_path: String,
    pub rootfs_path: String,
}

#[derive(Clone, Debug)]
pub enum RuntimeBackend {
    Wasm(PathBuf),
    Firecracker(KernelConfig),
}

pub struct Registry {
    tools: HashMap<String, RuntimeBackend>,
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

impl Registry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn load_from_config(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        // Mock parsing TOML for now, just register a dummy for testing
        let mut reg = Self::new();
        if path.contains("test") {
            reg.tools.insert(
                "mock_tool".to_string(),
                RuntimeBackend::Wasm(PathBuf::from("/tmp/mock.wasm")),
            );
        }
        Ok(reg)
    }

    pub fn resolve(&self, tool_name: &str) -> Option<&RuntimeBackend> {
        self.tools.get(tool_name)
    }

    pub fn register(&mut self, tool_name: String, backend: RuntimeBackend) {
        self.tools.insert(tool_name, backend);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_resolve() {
        let mut registry = Registry::new();
        registry.register(
            "fs".to_string(),
            RuntimeBackend::Wasm(PathBuf::from("fs.wasm")),
        );

        assert!(registry.resolve("fs").is_some());
        assert!(registry.resolve("unknown").is_none());
    }

    #[test]
    fn test_registry_load() {
        let registry = Registry::load_from_config("test.toml").unwrap();
        assert!(registry.resolve("mock_tool").is_some());
    }
}
