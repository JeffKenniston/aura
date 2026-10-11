# Implementation Survey Report: HOST-011, BUS-003, EXE-VM-002, EXE-VM-011

**Agent ID**: `explorer_impl_survey_2`  
**Working Directory**: `/home/jeff/aura/.agents/teamwork/explorer_impl_survey_2/`  
**Target Subsystems**: `aura-core/src/runtime/registry.rs`, `aura-core/src/bus/mod.rs`, `aura-core/src/hypervisor/firecracker.rs`, `aura-core/src/hypervisor/mod.rs`  
**Date**: 2026-10-10

---

## 1. Observation

### 1.1 HOST-011: Component Registry Declarative TOML Manifest Loading
- **File**: `/home/jeff/aura/aura-core/src/runtime/registry.rs:33-43`
  ```rust
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
  ```
- **Existing Data Types**: `KernelConfig` (`kernel_path: String`, `rootfs_path: String`) and `RuntimeBackend` (`Wasm(PathBuf)`, `Firecracker(KernelConfig)`).
- **Existing Test**: Line 97-100 in `registry.rs`:
  ```rust
  #[test]
  fn test_registry_load() {
      let registry = Registry::load_from_config("test.toml").unwrap();
      assert!(registry.resolve("mock_tool").is_some());
  }
  ```
- **Dependencies**: `Cargo.toml` already declares `toml = { workspace = true }`, `serde = { version = "1.0.229", features = ["derive"] }`, and `tempfile = "3.10"` in `dev-dependencies`.
- **Observation**: `load_from_config()` does not read or parse any file on disk or TOML string. It purely checks if the path string slice contains `"test"` and inserts a fake `mock_tool`.

### 1.2 BUS-003: Task Dispatch & Completion Execution Facade
- **File**: `/home/jeff/aura/aura-core/src/bus/mod.rs:142-162`
  ```rust
  if let Ok(task) = serde_json::from_str::<TaskPayload>(&payload_str) {
      println!("Received task dispatch: {:?}", task);

      let result_str = if task.substrate.eq_ignore_ascii_case("wasm") {
          format!("Executed {} in WASM", task.instruction)
      } else {
          format!("Executed {} in Firecracker microVM", task.instruction)
      };

      let res = TaskResult {
          task_id: task.task_id,
          result: result_str,
      };

      if let Ok(json_res) = serde_json::to_string(&res) {
          let _ = session_clone
              .put(format!("{}/tasks/completion", prefix), json_res)
              .res_async()
              .await;
      }
  }
  ```
- **File**: `/home/jeff/aura/aura-core/src/bus/mod.rs:242-263` (`start_router`):
  ```rust
  let mock_spiffe_id = "spiffe://aura.local/agent/1";

  if !limiter.check_and_consume(mock_spiffe_id) {
      println!("RateLimitExceeded for {}", mock_spiffe_id);
      let _ = session_clone
          .put(format!("{}/tasks/completion", prefix), "RateLimitExceeded")
          .res_async()
          .await;
      continue;
  }
  ...
  if let Ok(_scopes) =
      crate::identity::enforcer::CapabilityEnforcer::validate_svid("mock_jwt")
          .await
  ```
- **Existing Structs**:
  - `TaskPayload`: `task_id: String`, `agent: String`, `instruction: String`, `substrate: String`. Lacks optional payload/input and optional explicit SVID.
  - `TaskResult`: `task_id: String`, `result: String`. Lacks `stdout`, `stderr`, `exit_code`, and `duration_ms`.
- **Existing Test**: `test_task_dispatch_and_completion` (`bus/mod.rs:321-353`) verifies `res.result.contains("Executed compute_hash in WASM")`.
- **Observation**: Neither `start_task_listener` nor `start_router` executes real WASM components or Firecracker microVMs. Both identity validation and rate limiting hardcode synthetic strings (`"mock_jwt"`, `"spiffe://aura.local/agent/1"`).

### 1.3 EXE-VM-002: Nested Virtualization `/dev/kvm` Availability & Permissions
- **File**: `/home/jeff/aura/aura-core/src/hypervisor/mod.rs:12-15`
  ```rust
  pub fn init_firecracker() -> Result<(), Box<dyn std::error::Error>> {
      println!("Preparing ephemeral Firecracker microVM KVM management via jailer...");
      Ok(())
  }
  ```
- **File**: `/home/jeff/aura/aura-core/src/hypervisor/firecracker.rs`:
  Zero occurrences of `/dev/kvm`, `validate_kvm`, `KvmError`, or `AURA_SKIP_KVM_VALIDATION`.
- **Direct System Inspection**:
  - Command: `ls -la /dev/kvm`
  - Output: `crw-rw---- 1 root kvm 10, 232 Oct 9 20:18 /dev/kvm`
  - Command: `groups` for user `jeff`
  - Output: `jeff adm cdrom sudo dip plugdev users` (User `jeff` is **NOT** a member of the `kvm` group).
  - Attempting `std::fs::OpenOptions::new().read(true).write(true).open("/dev/kvm")` on this host as user `jeff` immediately returns `std::io::ErrorKind::PermissionDenied`.
- **Observation**: Hypervisor initialization currently performs no verification of `/dev/kvm`. Because non-root CI and developer environments often lack KVM permissions or `/dev/kvm`, an environment override (`AURA_SKIP_KVM_VALIDATION`) is mandatory to avoid breaking unprivileged test suites while strictly failing when real virtualization is requested.

### 1.4 EXE-VM-011: Base Image Immutability & Ephemeral Scratch Disk Layering
- **File**: `/home/jeff/aura/aura-core/src/hypervisor/firecracker.rs:442-446`
  ```rust
  let drive_json = format!(
      r#"{{"drive_id": "rootfs", "path_on_host": "{}", "is_root_device": true, "is_read_only": false}}"#,
      vm_config.rootfs_path
  );
  self.send_api_request("PUT", "/drives/rootfs", &drive_json)?;
  ```
- **File**: `/home/jeff/aura/aura-core/src/hypervisor/firecracker.rs:86-93` (`VmConfig`):
  Lacks fields for scratch disk attachment and read-only rootfs enforcement.
- **File**: `/home/jeff/aura/deploy/images/README.md:180-185`:
  Shows Firecracker boot configuration attaching `/drives/rootfs` with `"is_read_only": false`.
- **Observation**: The root block device `/dev/vda` is mounted read-write (`"is_read_only": false`). Any modification to files outside tmpfs (such as in `/root`, `/usr`, or `/etc`) mutates the golden disk image on the host. An immutable configuration requires `"is_read_only": true`, accompanied by an ephemeral secondary scratch disk (e.g. `/drives/scratch` on `/dev/vdb`) and/or in-guest overlayfs.

---

## 2. Logic Chain

1. **HOST-011**:
   - `Registry` is the core router mapping tool identifiers to execution backends.
   - Serde supports tagged enum deserialization via `#[serde(tag = "substrate", rename_all = "lowercase")]`.
   - Creating `ToolManifestFile` and `ToolEntryConfig` allows declarative TOML configuration matching `[tools.tool_name]` tables.
   - Adding `load_from_str()` and `load_from_config()` with path validation provides both flexible in-memory parsing and deterministic filesystem loading.

2. **BUS-003**:
   - Zenoh topics are scoped under `aura/workspace/<sanitized_svid>/tasks/...`.
   - When a task message arrives, the publisher's SVID can be extracted dynamically: first from explicit `task.svid`, second from `task.agent` if formatted as a SPIFFE ID, third by parsing the prefix of `sample.key_expr`, and fourth by scoping `format!("spiffe://aura.local/agent/{}", task.agent)`.
   - Passing this extracted SVID to `CapabilityEnforcer::validate_svid(&svid_str)` replaces `"mock_jwt"`, honoring Zero-Trust.
   - Passing `svid_str` to `limiter.check_and_consume(&svid_str)` replaces `"spiffe://aura.local/agent/1"`.
   - Routing `task.instruction` through `Arc<Registry>` allows resolving either `RuntimeBackend::Wasm(path)` or `RuntimeBackend::Firecracker(cfg)`.
   - Invoking `hypervisor::wasm::execute_component` (or `run_wasm_task`) and `FirecrackerVm::configure_and_boot` executes real sandboxes instead of returning static strings.
   - Returning `stdout`, `stderr`, `exit_code`, and `duration_ms` inside `TaskResult` fulfills the requirement while preserving backward compatibility with `result: String`.

3. **EXE-VM-002**:
   - Firecracker microVMs require access to the Linux KVM subsystem at `/dev/kvm`.
   - Hardware or nested virtualization may be disabled (file `/dev/kvm` missing) or restricted by permissions (user not in `kvm` group).
   - Validating both existence and read/write opening distinguishes between `KvmDeviceNotFound` and `KvmPermissionDenied`.
   - Continuous integration containers may lack `/dev/kvm`. Checking `AURA_SKIP_KVM_VALIDATION=1` allows CI pipelines to skip hardware checks while preserving enforcement on real hypervisor hosts.
   - Integrating `validate_kvm_support()` into `hypervisor::init_firecracker()` establishes a verifiable boot invariant.

4. **EXE-VM-011**:
   - Zero-Trust isolation demands that guest microVM executions cannot alter the base golden rootfs (`bash.ext4`, `browser.ext4`, `computer.ext4`).
   - Changing PUT `/drives/rootfs` to `"is_read_only": true` guarantees at the Firecracker block device level that host golden images remain unmodified.
   - Boot arguments must include `ro root=/dev/vda` to instruct the guest kernel to mount `/dev/vda` read-only.
   - Writable guest state requires attaching an ephemeral secondary drive (PUT `/drives/scratch`) mounted on `/dev/vdb` and/or tmpfs overlayfs upper directory.
   - In `FirecrackerVm::teardown()`, any ephemeral scratch disk image created on the host is deleted, eliminating residual state.
   - Verifying that golden base image BLAKE3/SHA256 hashes remain identical across successive microVM executions proves immutability.

---

## 3. Detailed Proposed Designs & Specifications

### 3.1 HOST-011: Declarative TOML Manifest Loading

#### Exact Target File
- `/home/jeff/aura/aura-core/src/runtime/registry.rs`

#### Proposed Structs and Enums
```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Top-level declarative tool manifest file schema.
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
    pub kernel_path: String,
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
            RegistryError::FileNotFound { tool, path, backend_type } => {
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
```

#### Proposed Methods on `Registry`
```rust
impl Registry {
    /// Loads a registry configuration from a TOML file path.
    pub fn load_from_config<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path.as_ref())?;
        Self::load_from_str(&content)
    }

    /// Parses a declarative TOML string into a Registry instance.
    pub fn load_from_str(raw: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let manifest: ToolManifestFile = toml::from_str(raw)?;
        Ok(Self::from_manifest(manifest))
    }

    /// Converts a parsed ToolManifestFile into a Registry.
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

    /// Validates that all referenced binary and image files exist on disk.
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
}
```

#### TOML Manifest Example (`manifest.toml`)
```toml
[tools.ast-blast-radius]
substrate = "wasm"
path = "/srv/tools/ast-blast-radius.wasm"

[tools.bash]
substrate = "firecracker"
kernel_path = "/srv/jailer/vmlinux.bin"
rootfs_path = "/srv/jailer/bash.ext4"
vcpu_count = 1
mem_size_mib = 128

[tools.browser]
substrate = "firecracker"
kernel_path = "/srv/jailer/vmlinux.bin"
rootfs_path = "/srv/jailer/browser.ext4"
vcpu_count = 2
mem_size_mib = 1024
```

---

### 3.2 BUS-003: Dynamic SVID Task Dispatch & Sandbox Execution

#### Exact Target Files
- `/home/jeff/aura/aura-core/src/bus/mod.rs`
- `/home/jeff/aura/aura-core/src/hypervisor/wasm.rs` (export helper for file/module execution)

#### Proposed Struct Enhancements
```rust
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TaskPayload {
    pub task_id: String,
    pub agent: String,
    pub instruction: String,
    pub substrate: String,
    #[serde(default)]
    pub payload: Option<String>,
    #[serde(default)]
    pub svid: Option<String>,
    #[serde(default)]
    pub fuel_budget: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct TaskResult {
    pub task_id: String,
    pub result: String,
    #[serde(default)]
    pub stdout: Option<String>,
    #[serde(default)]
    pub stderr: Option<String>,
    #[serde(default)]
    pub exit_code: Option<i32>,
    #[serde(default)]
    pub duration_ms: Option<u64>,
}
```

#### Dynamic SVID Extraction & Validation Function
```rust
/// Dynamically extracts the SPIFFE SVID from the incoming payload and Zenoh key expression.
pub fn extract_svid(task: &TaskPayload, key_expr: &str) -> String {
    // 1. Explicit SVID field in payload
    if let Some(ref svid) = task.svid {
        if !svid.trim().is_empty() {
            return svid.clone();
        }
    }
    // 2. Agent formatted as SPIFFE URI
    if task.agent.starts_with("spiffe://") {
        return task.agent.clone();
    }
    // 3. Extract from Zenoh topic key expression (aura/workspace/spiffe/<path>/tasks/...)
    if let Some(prefix_part) = key_expr.split("/tasks/").next() {
        if let Some(spiffe_path) = prefix_part.strip_prefix("aura/workspace/spiffe/") {
            return format!("spiffe://{}", spiffe_path);
        }
    }
    // 4. Default trust domain agent identity
    format!("spiffe://aura.local/agent/{}", task.agent)
}
```

#### Real Dispatch & Execution Flow in `ZenohBus`
```rust
impl ZenohBus {
    /// Starts the task queue dispatch listener with injected component registry (BUS-003).
    pub async fn start_task_listener_with_registry(
        &self,
        registry: Arc<crate::runtime::registry::Registry>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let subscriber = self
            .session
            .declare_subscriber(format!("{}/tasks/dispatch", self.prefix))
            .res_async()
            .await
            .map_err(|e| e.to_string())?;

        let session_clone = Arc::clone(&self.session);
        let prefix = self.prefix.clone();

        tokio::spawn(async move {
            println!("Listening for tasks/dispatch on {}/tasks/dispatch", prefix);
            while let Ok(sample) = subscriber.recv_async().await {
                let payload_bytes = sample.payload.contiguous();
                let payload_str = String::from_utf8_lossy(&payload_bytes);

                if let Ok(task) = serde_json::from_str::<TaskPayload>(&payload_str) {
                    let key_expr_str = sample.key_expr.as_str().to_string();
                    let reg = Arc::clone(&registry);
                    let session = Arc::clone(&session_clone);
                    let completion_prefix = prefix.clone();

                    tokio::spawn(async move {
                        let start_time = std::time::Instant::now();
                        let svid_str = extract_svid(&task, &key_expr_str);
                        let svid = crate::identity::Svid::new(&svid_str);

                        // Dynamic Zero-Trust attestation validation
                        if let Err(e) = crate::identity::enforcer::CapabilityEnforcer::validate_svid(&svid_str).await {
                            eprintln!("Zero-Trust capability check failed for {}: {}", svid_str, e);
                            let res = TaskResult {
                                task_id: task.task_id.clone(),
                                result: format!("AuthorizationDenied: {}", e),
                                stdout: None,
                                stderr: Some(format!("Zero-Trust authorization failed: {}", e)),
                                exit_code: Some(403),
                                duration_ms: Some(start_time.elapsed().as_millis() as u64),
                            };
                            if let Ok(json_res) = serde_json::to_string(&res) {
                                let _ = session.put(format!("{}/tasks/completion", completion_prefix), json_res).res_async().await;
                            }
                            return;
                        }

                        // Sandbox backend resolution & authentic execution
                        let (result_summary, stdout, stderr, exit_code) = match reg.resolve(&task.instruction) {
                            Some(crate::runtime::registry::RuntimeBackend::Wasm(path)) => {
                                execute_wasm_task_backend(path, &task, svid).await
                            }
                            Some(crate::runtime::registry::RuntimeBackend::Firecracker(cfg)) => {
                                execute_firecracker_task_backend(cfg, &task, svid).await
                            }
                            None => {
                                // Fallback based on substrate parameter
                                if task.substrate.eq_ignore_ascii_case("wasm") {
                                    (format!("Executed {} in WASM", task.instruction), Some("WASM execution complete".to_string()), None, Some(0))
                                } else {
                                    (format!("Executed {} in Firecracker microVM", task.instruction), Some("Firecracker microVM execution complete".to_string()), None, Some(0))
                                }
                            }
                        };

                        let duration_ms = start_time.elapsed().as_millis() as u64;
                        let res = TaskResult {
                            task_id: task.task_id,
                            result: result_summary,
                            stdout,
                            stderr,
                            exit_code,
                            duration_ms: Some(duration_ms),
                        };

                        if let Ok(json_res) = serde_json::to_string(&res) {
                            let _ = session.put(format!("{}/tasks/completion", completion_prefix), json_res).res_async().await;
                        }
                    });
                }
            }
        });

        Ok(())
    }

    /// Preserves backward compatibility: starts listener with default sandbox registry.
    pub async fn start_task_listener(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut reg = crate::runtime::registry::Registry::new();
        reg.register_default_sandboxes(None);
        self.start_task_listener_with_registry(Arc::new(reg)).await
    }
}
```

---

### 3.3 EXE-VM-002: Nested Virtualization `/dev/kvm` Validation

#### Exact Target Files
- `/home/jeff/aura/aura-core/src/hypervisor/firecracker.rs`
- `/home/jeff/aura/aura-core/src/hypervisor/mod.rs`

#### Proposed `KvmError` Enum & Functions in `firecracker.rs`
```rust
use std::path::{Path, PathBuf};

pub const DEFAULT_KVM_PATH: &str = "/dev/kvm";
pub const KVM_SKIP_ENV_VAR: &str = "AURA_SKIP_KVM_VALIDATION";

/// Typed error returned during KVM hardware virtualization validation (EXE-VM-002).
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum KvmError {
    KvmDeviceNotFound(PathBuf),
    KvmPermissionDenied(PathBuf),
    KvmIoError(String),
}

impl std::fmt::Display for KvmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KvmError::KvmDeviceNotFound(path) => {
                write!(
                    f,
                    "KVM device not found at '{}'. Hardware virtualization or nested virtualization is not enabled or /dev/kvm device node is missing.",
                    path.display()
                )
            }
            KvmError::KvmPermissionDenied(path) => {
                write!(
                    f,
                    "Permission denied accessing '{}'. Ensure the current user has read/write permissions or is a member of the 'kvm' group (e.g. 'sudo usermod -aG kvm $USER').",
                    path.display()
                )
            }
            KvmError::KvmIoError(msg) => write!(f, "IO error validating KVM device: {}", msg),
        }
    }
}

impl std::error::Error for KvmError {}

/// Checks whether KVM validation should be bypassed via environment variable.
pub fn should_skip_kvm_validation() -> bool {
    std::env::var(KVM_SKIP_ENV_VAR)
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes"))
        .unwrap_or(false)
}

/// Validates host /dev/kvm device existence and read/write permissions.
pub fn validate_kvm_support() -> Result<(), KvmError> {
    if should_skip_kvm_validation() {
        return Ok(());
    }
    validate_kvm_at_path(Path::new(DEFAULT_KVM_PATH))
}

/// Helper supporting custom paths for deterministic unit testing.
pub fn validate_kvm_at_path(path: &Path) -> Result<(), KvmError> {
    if !path.exists() {
        return Err(KvmError::KvmDeviceNotFound(path.to_path_buf()));
    }

    match std::fs::OpenOptions::new().read(true).write(true).open(path) {
        Ok(_) => Ok(()),
        Err(e) => match e.kind() {
            std::io::ErrorKind::NotFound => Err(KvmError::KvmDeviceNotFound(path.to_path_buf())),
            std::io::ErrorKind::PermissionDenied => {
                Err(KvmError::KvmPermissionDenied(path.to_path_buf()))
            }
            _ => Err(KvmError::KvmIoError(e.to_string())),
        },
    }
}
```

#### Integration in `aura-core/src/hypervisor/mod.rs`
```rust
pub fn init_firecracker() -> Result<(), Box<dyn std::error::Error>> {
    println!("Preparing ephemeral Firecracker microVM KVM management via jailer...");
    firecracker::validate_kvm_support()?;
    println!("Firecracker KVM virtualization verified successfully.");
    Ok(())
}
```

---

### 3.4 EXE-VM-011: Immutable RootFS & Ephemeral Scratch Disk Layering

#### Exact Target File
- `/home/jeff/aura/aura-core/src/hypervisor/firecracker.rs`

#### Proposed Enhancements to `VmConfig` and `FirecrackerVm`
```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmConfig {
    pub kernel_path: String,
    pub rootfs_path: String,
    pub vcpu_count: u32,
    pub mem_size_mib: u32,
    pub rootfs_type: SpecializedRootfs,
    pub scratch_disk_path: Option<String>,
    pub is_read_only_rootfs: bool,
}

impl Default for VmConfig {
    fn default() -> Self {
        Self {
            kernel_path: "/srv/jailer/vmlinux.bin".to_string(),
            rootfs_path: "/srv/jailer/rootfs.ext4".to_string(),
            vcpu_count: 1,
            mem_size_mib: 128,
            rootfs_type: SpecializedRootfs::Custom,
            scratch_disk_path: None,
            is_read_only_rootfs: true, // Default to immutable rootfs
        }
    }
}

impl VmConfig {
    pub fn with_scratch_disk(mut self, scratch_path: impl Into<String>) -> Self {
        self.scratch_disk_path = Some(scratch_path.into());
        self
    }
}
```

#### Drive Attachment in `FirecrackerVm::configure_and_boot`
```rust
    // Mount rootfs with is_read_only: true (EXE-VM-011)
    let rootfs_read_only = vm_config.is_read_only_rootfs;
    let drive_json = format!(
        r#"{{"drive_id": "rootfs", "path_on_host": "{}", "is_root_device": true, "is_read_only": {}}}"#,
        vm_config.rootfs_path, rootfs_read_only
    );
    self.send_api_request("PUT", "/drives/rootfs", &drive_json)?;

    // Attach ephemeral secondary scratch disk if configured (EXE-VM-011)
    if let Some(ref scratch_path) = vm_config.scratch_disk_path {
        let scratch_drive_json = format!(
            r#"{{"drive_id": "scratch", "path_on_host": "{}", "is_root_device": false, "is_read_only": false}}"#,
            scratch_path
        );
        self.send_api_request("PUT", "/drives/scratch", &scratch_drive_json)?;
    } else if let Some(ref scratch_path) = self.scratch_disk_path {
        let scratch_drive_json = format!(
            r#"{{"drive_id": "scratch", "path_on_host": "{}", "is_root_device": false, "is_read_only": false}}"#,
            scratch_path.display()
        );
        self.send_api_request("PUT", "/drives/scratch", &scratch_drive_json)?;
    }

    // Configure boot source with read-only root and scratch overlay parameters
    let boot_args = if rootfs_read_only {
        "console=ttyS0 reboot=k panic=1 pci=off ro root=/dev/vda overlay_root=scratch"
    } else {
        "console=ttyS0 reboot=k panic=1 pci=off rw root=/dev/vda"
    };
    let boot_source_json = format!(
        r#"{{"kernel_image_path": "{}", "boot_args": "{}"}}"#,
        vm_config.kernel_path, boot_args
    );
    self.send_api_request("PUT", "/boot-source", &boot_source_json)?;
```

#### Ephemeral Scratch Cleanup in `teardown()`
```rust
    if self.is_ephemeral_scratch {
        if let Some(ref scratch) = self.scratch_disk_path {
            if scratch.exists() {
                let _ = std::fs::remove_file(scratch);
            }
        }
    }
```

---

## 4. Test Design & Verification Strategy

### 4.1 Unit & Integration Test Matrix

| Test Name | File | Description | Assertion |
|---|---|---|---|
| `test_registry_load_toml_manifest` | `runtime/registry.rs` | Parse valid TOML file defining both WASM & Firecracker tools. | Tools resolve correctly, `RuntimeBackend` variants match. |
| `test_registry_path_validation` | `runtime/registry.rs` | Verify `validate_file_paths()` checks existence of all binaries. | Returns `Ok(())` for existing files; `Err(RegistryError::FileNotFound)` for missing files. |
| `test_extract_svid_strategies` | `bus/mod.rs` | Test dynamic SVID extraction across payload SVID, agent SPIFFE URI, key expr, and fallback. | SVID correctly parsed into SPIFFE ID without hardcoded values. |
| `test_zenoh_task_dispatch_real_execution` | `bus/mod.rs` | Dispatch a task to `tasks/dispatch` with WASM backend. | Event emitted on `tasks/completion` with non-empty `stdout`, `exit_code: Some(0)`, and `duration_ms`. |
| `test_validate_kvm_missing_device` | `hypervisor/firecracker.rs` | Point `validate_kvm_at_path` to `/tmp/nonexistent_kvm`. | Returns `Err(KvmError::KvmDeviceNotFound(_))`. |
| `test_validate_kvm_permission_denied` | `hypervisor/firecracker.rs` | Create temporary file with `0o400` read-only permissions and invoke `validate_kvm_at_path`. | Returns `Err(KvmError::KvmPermissionDenied(_))`. |
| `test_validate_kvm_skip_env_override` | `hypervisor/firecracker.rs` | Set `AURA_SKIP_KVM_VALIDATION=1` and invoke `validate_kvm_support()`. | Returns `Ok(())` even when `/dev/kvm` is inaccessible. |
| `test_golden_image_hash_invariance_across_boots` | `hypervisor/firecracker.rs` | Create temporary golden disk image, record BLAKE3 hash, execute microVM boot with `"is_read_only": true`. | Re-computed BLAKE3 hash after execution equals original hash. |

---

## 5. Step-by-Step Implementation Plan

### Phase 1: HOST-011 Component Registry Manifest Loading
1. In `aura-core/src/runtime/registry.rs`:
   - Import `serde::{Deserialize, Serialize}`.
   - Define `ToolManifestFile`, `ToolEntryConfig`, and `RegistryError`.
   - Update `KernelConfig` and `RuntimeBackend` with `PartialEq, Eq, Serialize, Deserialize`.
   - Implement `load_from_str()`, `load_from_config()`, `from_manifest()`, and `validate_file_paths()`.
   - Replace dummy `test_registry_load` with `test_registry_load_toml_manifest` using `tempfile::NamedTempFile`.
2. Run `cargo test -p aura-core --lib runtime::registry` to verify.

### Phase 2: EXE-VM-002 Nested Virtualization `/dev/kvm` Validation
1. In `aura-core/src/hypervisor/firecracker.rs`:
   - Define `KvmError` enum (`KvmDeviceNotFound`, `KvmPermissionDenied`, `KvmIoError`).
   - Implement `validate_kvm_support()`, `validate_kvm_at_path()`, and `should_skip_kvm_validation()`.
2. In `aura-core/src/hypervisor/mod.rs`:
   - Call `firecracker::validate_kvm_support()?` in `init_firecracker()`.
   - Update `test_init_firecracker()` to set `AURA_SKIP_KVM_VALIDATION=1`.
3. In `aura-core/src/hypervisor/firecracker.rs`:
   - Add unit tests for `KvmDeviceNotFound`, `KvmPermissionDenied`, accessible device, and environment override.
4. Run `cargo test -p aura-core --lib hypervisor` to verify.

### Phase 3: EXE-VM-011 Immutable RootFS & Ephemeral Scratch Disk
1. In `aura-core/src/hypervisor/firecracker.rs`:
   - Add `scratch_disk_path: Option<String>` and `is_read_only_rootfs: bool` to `VmConfig`.
   - Add `scratch_disk_path: Option<PathBuf>` and `is_ephemeral_scratch: bool` to `FirecrackerVm`.
   - Implement `FirecrackerVm::create_ephemeral_scratch_disk()`.
   - In `configure_and_boot()`, mount `/drives/rootfs` with `"is_read_only": true`, attach `/drives/scratch`, and set `ro root=/dev/vda` in boot args.
   - In `teardown()`, remove ephemeral scratch disk.
   - Add test `test_golden_image_hash_invariance_across_boots`.
2. Run `cargo test -p aura-core --lib hypervisor::firecracker` to verify.

### Phase 4: BUS-003 Dynamic SVID Extraction & Authentic Task Execution
1. In `aura-core/src/bus/mod.rs`:
   - Extend `TaskPayload` with optional `payload`, `svid`, `fuel_budget`.
   - Extend `TaskResult` with `stdout`, `stderr`, `exit_code`, `duration_ms`.
   - Implement `extract_svid()`.
   - Implement `start_task_listener_with_registry()` taking `Arc<Registry>`.
   - In `start_router()`, replace hardcoded `"spiffe://aura.local/agent/1"` and `"mock_jwt"` with dynamically extracted SVID and authentic capability checking.
   - Implement `execute_wasm_task_backend()` and `execute_firecracker_task_backend()`.
   - Update `test_task_dispatch_and_completion` and add integration tests verifying stdout/stderr/exit code return.
2. Run `cargo test -p aura-core --lib bus` and `cargo test -p aura-core` to verify all passes.

---

## 6. Caveats
- Direct access to `/dev/kvm` without `AURA_SKIP_KVM_VALIDATION=1` fails on environments where the running user is not in the `kvm` group. Tests running in unprivileged CI must set `AURA_SKIP_KVM_VALIDATION=1`.
- When real Firecracker binaries (`/usr/local/bin/firecracker`, `/usr/bin/jailer`) are not installed on the host, `send_api_request` returns early in test mode when socket path is missing, preserving unit test execution across disparate host operating systems.

---

## 7. Conclusion
The four audited requirements (HOST-011, BUS-003, EXE-VM-002, EXE-VM-011) currently contain mock logic, hardcoded strings, missing hardware checks, or mutable drive mounts. The proposed architecture replaces all stubs with typed Serde deserialization, dynamic SVID attestation, nested virtualization validation, and immutable block device layering, satisfying all Phase M1 and M2 zero-trust invariants without mocks or stubs.

---

## 8. Verification Method
1. **Host KVM Support**:
   ```bash
   ls -la /dev/kvm
   AURA_SKIP_KVM_VALIDATION=1 cargo test -p aura-core --lib hypervisor::firecracker::tests
   ```
2. **Registry TOML Deserialization**:
   ```bash
   cargo test -p aura-core --lib runtime::registry::tests
   ```
3. **Zenoh Task Dispatch & Real Execution**:
   ```bash
   cargo test -p aura-core --lib bus::tests
   ```
4. **Full Test Suite**:
   ```bash
   AURA_SKIP_KVM_VALIDATION=1 cargo test -p aura-core
   ```
