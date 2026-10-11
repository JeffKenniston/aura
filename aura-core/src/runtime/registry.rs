use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Top-level declarative tool manifest file schema (HOST-011).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolManifestFile {
    pub tools: HashMap<String, ToolEntryConfig>,
}

/// Individual tool execution substrate configuration entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "substrate", rename_all = "lowercase")]
pub enum ToolEntryConfig {
    Wasm {
        path: PathBuf,
    },
    Firecracker {
        #[serde(alias = "kernel")]
        kernel_path: String,
        #[serde(alias = "rootfs")]
        rootfs_path: String,
        #[serde(default)]
        vcpu_count: Option<u32>,
        #[serde(default)]
        mem_size_mib: Option<u32>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KernelConfig {
    #[serde(alias = "kernel")]
    pub kernel_path: String,
    #[serde(alias = "rootfs")]
    pub rootfs_path: String,
    #[serde(default)]
    pub vcpu_count: Option<u32>,
    #[serde(default)]
    pub mem_size_mib: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeBackend {
    Wasm(PathBuf),
    Firecracker(KernelConfig),
}

#[derive(Debug, PartialEq, Eq)]
pub enum RegistryError {
    FileNotFound {
        tool: String,
        path: String,
        backend_type: String,
    },
    ParseError(String),
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegistryError::FileNotFound {
                tool,
                path,
                backend_type,
            } => {
                write!(
                    f,
                    "Registry validation error: tool '{}' ({}) file not found at '{}'",
                    tool, backend_type, path
                )
            }
            RegistryError::ParseError(msg) => write!(f, "Registry manifest parse error: {}", msg),
        }
    }
}

impl std::error::Error for RegistryError {}

#[derive(Clone, Debug, Default)]
pub struct Registry {
    tools: HashMap<String, RuntimeBackend>,
}

impl Registry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Loads a declarative tool manifest from a TOML file on disk (HOST-011).
    pub fn load_from_config<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let raw = std::fs::read_to_string(path)?;
        Self::load_from_str(&raw)
    }

    /// Parses a declarative TOML string into a Registry instance.
    pub fn load_from_str(raw: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let manifest: ToolManifestFile = toml::from_str(raw)?;
        Ok(Self::from_manifest(manifest))
    }

    /// Constructs a Registry from a parsed ToolManifestFile.
    pub fn from_manifest(manifest: ToolManifestFile) -> Self {
        let mut reg = Self::new();
        for (name, entry) in manifest.tools {
            match entry {
                ToolEntryConfig::Wasm { path } => {
                    reg.register(name, RuntimeBackend::Wasm(path));
                }
                ToolEntryConfig::Firecracker {
                    kernel_path,
                    rootfs_path,
                    vcpu_count,
                    mem_size_mib,
                } => {
                    reg.register(
                        name,
                        RuntimeBackend::Firecracker(KernelConfig {
                            kernel_path,
                            rootfs_path,
                            vcpu_count,
                            mem_size_mib,
                        }),
                    );
                }
            }
        }
        reg
    }

    /// Validates that all referenced binary and rootfs files exist on disk.
    pub fn validate_file_paths(&self) -> Result<(), RegistryError> {
        for (name, backend) in &self.tools {
            match backend {
                RuntimeBackend::Wasm(path) => {
                    if !path.exists() {
                        return Err(RegistryError::FileNotFound {
                            tool: name.clone(),
                            path: path.display().to_string(),
                            backend_type: "wasm".to_string(),
                        });
                    }
                }
                RuntimeBackend::Firecracker(cfg) => {
                    let kpath = Path::new(&cfg.kernel_path);
                    if !kpath.exists() {
                        return Err(RegistryError::FileNotFound {
                            tool: name.clone(),
                            path: cfg.kernel_path.clone(),
                            backend_type: "firecracker_kernel".to_string(),
                        });
                    }
                    let rpath = Path::new(&cfg.rootfs_path);
                    if !rpath.exists() {
                        return Err(RegistryError::FileNotFound {
                            tool: name.clone(),
                            path: cfg.rootfs_path.clone(),
                            backend_type: "firecracker_rootfs".to_string(),
                        });
                    }
                }
            }
        }
        Ok(())
    }

    pub fn resolve(&self, tool_name: &str) -> Option<&RuntimeBackend> {
        self.tools.get(tool_name)
    }

    pub fn register(&mut self, tool_name: String, backend: RuntimeBackend) {
        self.tools.insert(tool_name, backend);
    }

    pub fn tools(&self) -> &HashMap<String, RuntimeBackend> {
        &self.tools
    }

    pub fn contains_tool(&self, tool_name: &str) -> bool {
        self.tools.contains_key(tool_name)
    }

    /// Registers default sandbox tool backends based on specialized rootfs images (M2 Sandboxes).
    pub fn register_default_sandboxes(&mut self, base_dir: Option<&str>) {
        let base = base_dir.unwrap_or("/srv/jailer");
        self.register(
            "bash".to_string(),
            RuntimeBackend::Firecracker(KernelConfig {
                kernel_path: format!("{}/vmlinux.bin", base.trim_end_matches('/')),
                rootfs_path: format!("{}/bash.ext4", base.trim_end_matches('/')),
                vcpu_count: Some(1),
                mem_size_mib: Some(128),
            }),
        );
        self.register(
            "browser".to_string(),
            RuntimeBackend::Firecracker(KernelConfig {
                kernel_path: format!("{}/vmlinux.bin", base.trim_end_matches('/')),
                rootfs_path: format!("{}/browser.ext4", base.trim_end_matches('/')),
                vcpu_count: Some(2),
                mem_size_mib: Some(1024),
            }),
        );
        self.register(
            "computer".to_string(),
            RuntimeBackend::Firecracker(KernelConfig {
                kernel_path: format!("{}/vmlinux.bin", base.trim_end_matches('/')),
                rootfs_path: format!("{}/computer.ext4", base.trim_end_matches('/')),
                vcpu_count: Some(2),
                mem_size_mib: Some(1024),
            }),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

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
    fn test_registry_load_valid_toml() {
        let toml_content = r#"
[tools.ast-blast-radius]
substrate = "wasm"
path = "/srv/tools/ast-blast-radius.wasm"

[tools.bash_executor]
substrate = "firecracker"
kernel_path = "/srv/jailer/vmlinux.bin"
rootfs_path = "/srv/jailer/bash.ext4"
vcpu_count = 2
mem_size_mib = 512

[tools.browser_tool]
substrate = "firecracker"
kernel = "/srv/jailer/vmlinux.bin"
rootfs = "/srv/jailer/browser.ext4"
"#;

        // Verify in-memory load_from_str
        let registry_from_str =
            Registry::load_from_str(toml_content).expect("load_from_str should succeed");
        assert_eq!(registry_from_str.tools().len(), 3);

        // Verify WASM tool resolution
        let wasm_tool = registry_from_str
            .resolve("ast-blast-radius")
            .expect("ast-blast-radius tool should be resolved");
        match wasm_tool {
            RuntimeBackend::Wasm(path) => {
                assert_eq!(path, &PathBuf::from("/srv/tools/ast-blast-radius.wasm"));
            }
            _ => panic!("Expected WASM backend for ast-blast-radius"),
        }

        // Verify Firecracker tool with kernel_path/rootfs_path and optional specs
        let fc_tool = registry_from_str
            .resolve("bash_executor")
            .expect("bash_executor tool should be resolved");
        match fc_tool {
            RuntimeBackend::Firecracker(cfg) => {
                assert_eq!(cfg.kernel_path, "/srv/jailer/vmlinux.bin");
                assert_eq!(cfg.rootfs_path, "/srv/jailer/bash.ext4");
                assert_eq!(cfg.vcpu_count, Some(2));
                assert_eq!(cfg.mem_size_mib, Some(512));
            }
            _ => panic!("Expected Firecracker backend for bash_executor"),
        }

        // Verify Firecracker tool with serde aliases (kernel, rootfs)
        let browser_tool = registry_from_str
            .resolve("browser_tool")
            .expect("browser_tool should be resolved");
        match browser_tool {
            RuntimeBackend::Firecracker(cfg) => {
                assert_eq!(cfg.kernel_path, "/srv/jailer/vmlinux.bin");
                assert_eq!(cfg.rootfs_path, "/srv/jailer/browser.ext4");
                assert_eq!(cfg.vcpu_count, None);
                assert_eq!(cfg.mem_size_mib, None);
            }
            _ => panic!("Expected Firecracker backend for browser_tool"),
        }

        // Verify load_from_config reading from a real file on disk
        let mut temp_file = NamedTempFile::new().expect("Failed to create temporary file");
        temp_file
            .write_all(toml_content.as_bytes())
            .expect("Failed to write TOML to temp file");
        let registry_from_file =
            Registry::load_from_config(temp_file.path()).expect("load_from_config should succeed");
        assert_eq!(registry_from_file.tools().len(), 3);
        assert!(registry_from_file.resolve("ast-blast-radius").is_some());
        assert!(registry_from_file.resolve("bash_executor").is_some());
        assert!(registry_from_file.resolve("browser_tool").is_some());
    }

    #[test]
    fn test_registry_load_invalid_toml() {
        // Malformed TOML syntax
        let malformed_toml = "tools: { not a valid toml syntax [[";
        let result = Registry::load_from_str(malformed_toml);
        assert!(result.is_err(), "Malformed TOML must return an error");

        // Missing required field 'substrate'
        let missing_substrate = r#"
[tools.invalid_tool]
path = "/some/path"
"#;
        let result2 = Registry::load_from_str(missing_substrate);
        assert!(result2.is_err(), "Missing substrate tag must return an error");

        // Unknown substrate type
        let unknown_substrate = r#"
[tools.invalid_tool]
substrate = "docker"
image = "ubuntu:latest"
"#;
        let result3 = Registry::load_from_str(unknown_substrate);
        assert!(result3.is_err(), "Unknown substrate must return an error");
    }

    #[test]
    fn test_registry_validate_file_paths() {
        // Non-existent paths should fail validation
        let invalid_toml = r#"
[tools.nonexistent_wasm]
substrate = "wasm"
path = "/tmp/surely_nonexistent_binary_xyz_12345.wasm"
"#;
        let registry = Registry::load_from_str(invalid_toml).unwrap();
        let val_res = registry.validate_file_paths();
        assert!(val_res.is_err());
        match val_res.err().unwrap() {
            RegistryError::FileNotFound {
                tool,
                path,
                backend_type,
            } => {
                assert_eq!(tool, "nonexistent_wasm");
                assert_eq!(path, "/tmp/surely_nonexistent_binary_xyz_12345.wasm");
                assert_eq!(backend_type, "wasm");
            }
            other => panic!("Unexpected error variant: {:?}", other),
        }

        // Existing paths should pass validation
        let temp_wasm = NamedTempFile::new().unwrap();
        let temp_kernel = NamedTempFile::new().unwrap();
        let temp_rootfs = NamedTempFile::new().unwrap();

        let mut valid_registry = Registry::new();
        valid_registry.register(
            "real_wasm".to_string(),
            RuntimeBackend::Wasm(temp_wasm.path().to_path_buf()),
        );
        valid_registry.register(
            "real_fc".to_string(),
            RuntimeBackend::Firecracker(KernelConfig {
                kernel_path: temp_kernel.path().display().to_string(),
                rootfs_path: temp_rootfs.path().display().to_string(),
                vcpu_count: Some(1),
                mem_size_mib: Some(128),
            }),
        );

        assert!(
            valid_registry.validate_file_paths().is_ok(),
            "Validation must succeed when all files exist on disk"
        );
    }

    #[test]
    fn test_registry_default_sandboxes() {
        let mut registry = Registry::new();
        registry.register_default_sandboxes(Some("/srv/jailer/images"));

        let bash = registry
            .resolve("bash")
            .expect("bash tool should be registered");
        match bash {
            RuntimeBackend::Firecracker(cfg) => {
                assert_eq!(cfg.rootfs_path, "/srv/jailer/images/bash.ext4");
            }
            _ => panic!("Expected Firecracker backend for bash"),
        }

        let browser = registry
            .resolve("browser")
            .expect("browser tool should be registered");
        match browser {
            RuntimeBackend::Firecracker(cfg) => {
                assert_eq!(cfg.rootfs_path, "/srv/jailer/images/browser.ext4");
            }
            _ => panic!("Expected Firecracker backend for browser"),
        }

        let computer = registry
            .resolve("computer")
            .expect("computer tool should be registered");
        match computer {
            RuntimeBackend::Firecracker(cfg) => {
                assert_eq!(cfg.rootfs_path, "/srv/jailer/images/computer.ext4");
            }
            _ => panic!("Expected Firecracker backend for computer"),
        }
    }
}
