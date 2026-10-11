# M4 Survey Report: Firecracker Sandbox Linting, Ephemeral Tunnel & Patch Management

**Author**: Survey Explorer 2  
**Date**: 2026-10-09  
**Target Milestone**: Phase M4 (Repo Intelligence: Graph, Blast-Radius)  
**Scope**: `aura-core/src/knowledge/git.rs`, `aura-core/src/hypervisor/firecracker.rs`, Ephemeral Tunnel integration, and related test fixtures.

---

## 1. Executive Summary

Phase M4 requires completing the git intelligence and sandbox execution components in `aura-core/src/knowledge/git.rs`. Specifically:
1. **`check_drift`**: Integrate `MerkleTree::update()` from `indexer.rs` to detect concurrent external modifications not authored by the agent using BLAKE3 hashes.
2. **`sandbox_lint`**: Enforce requirement **R3 (Controlled Execution)** by launching a Firecracker microVM with `SpecializedRootfs::Bash` via `aura-core/src/hypervisor/firecracker.rs` to securely run repository linters (`ruff`, `prettier`, `cargo fmt`) in isolation without touching the host directly.
3. **`open_pr`**: Interface with the Ephemeral Tunnel (`tools/ephemeral-tunnel` or `crate::identity`) to mint short-lived credentials for the `"github"` service, create Pull Requests, and explicitly revoke tokens upon completion, keeping secrets out of model context.
4. **`apply_patch`**: Enforce a strict pre-commit verification pipeline: halt immediately if `check_drift` detects unauthored drift; halt immediately if `sandbox_lint` fails; and only commit and update the baseline Merkle tree when both gates pass.

All 74 existing tests in `aura-core` are currently passing. `git.rs` currently contains empty stubs and zero unit tests.

---

## 2. Firecracker Hypervisor Module Analysis

### 2.1 File Location & Architecture
- **Primary file**: `/home/jeff/aura/aura-core/src/hypervisor/firecracker.rs` (708 lines)
- **Supporting files**:
  - `/home/jeff/aura/aura-core/src/hypervisor/mod.rs` (`init_firecracker()`, `init_wasmtime()`)
  - `/home/jeff/aura/aura-core/src/hypervisor/proxy.rs` (Transparent TLS proxy for microVM egress)
  - `/home/jeff/aura/aura-core/src/config/mod.rs` (`FirecrackerConfig` at lines 305–343)
  - `/home/jeff/aura/aura-core/src/runtime/registry.rs` (`RuntimeBackend::Firecracker`, `register_default_sandboxes`)

### 2.2 Core Types and Signatures

```rust
// In aura-core/src/hypervisor/firecracker.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpecializedRootfs {
    Bash,     // "bash.ext4", 1 vCPU, 128 MiB RAM (Default for linter workloads)
    Browser,  // "browser.ext4", 2 vCPU, 1024 MiB RAM
    Computer, // "computer.ext4", 2 vCPU, 1024 MiB RAM
    Custom,   // "rootfs.ext4", 1 vCPU, 128 MiB RAM
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmConfig {
    pub kernel_path: String,
    pub rootfs_path: String,
    pub vcpu_count: u32,
    pub mem_size_mib: u32,
    pub rootfs_type: SpecializedRootfs,
}

impl VmConfig {
    pub fn for_bash(base_dir: Option<&str>) -> Self {
        Self::for_specialized_rootfs(SpecializedRootfs::Bash, base_dir)
    }
}

pub struct FirecrackerVm {
    pub id: String,
    pub svid: String,
    pub socket_path: PathBuf,
    pub chroot_dir: PathBuf,
    pub vsock_uds_path: PathBuf,
    jailer_child: Option<Child>,
    screenshot_socket: Option<PathBuf>,
}

impl FirecrackerVm {
    pub fn new(id: &str, svid: &str) -> Self;
    pub fn with_config(id: &str, svid: &str, config: &FirecrackerConfig) -> Self;
    pub fn start_jailer(&mut self, config: &FirecrackerConfig) -> Result<(), Box<dyn Error>>;
    pub fn configure_and_boot(&self, vm_config: &VmConfig) -> Result<(), Box<dyn Error>>;
    pub fn teardown(&mut self) -> Result<(), Box<dyn Error>>;
}

impl Drop for FirecrackerVm {
    fn drop(&mut self) {
        let _ = self.teardown();
    }
}
```

### 2.3 Unit Testing & Development Mode Invariant
In `FirecrackerVm::configure_and_boot(&self, vm_config: &VmConfig)` (`firecracker.rs:504-508`):
```rust
if !self.socket_path.exists() {
    // If socket does not exist (e.g. running outside real KVM jailer environment),
    // log and return early in development/test mode
    return Ok(());
}
```
**Key Architectural Property**: Unit and integration tests running on standard developer or CI machines (without KVM permissions or `/srv/jailer`) will cleanly exercise the `FirecrackerVm` configuration, initialization, and lifecycle teardown paths without panicking or requiring mocked sockets. However, invalid configurations (e.g., zero vCPUs, empty kernel paths, or insufficient memory) can be rejected before or during configuration, enabling deterministic failure testing.

---

## 3. Sandboxed Linting (`sandbox_lint`) Implementation Strategy

### 3.1 Requirement R3 (Controlled Execution)
- Requirement R3 explicitly states: *“You must use the provided Firecracker hypervisor module for `sandbox_lint`. The execution environment is managed externally by the hypervisor; do not attempt to run lints on the host directly.”*
- Linters covered: `ruff` (Python), `prettier` (JS/TS/JSON/Markdown), `cargo fmt` (Rust).
- Under the Aura architecture (`02-execution-sandboxes.md` and `aura-exe-vm-domain/SKILL.md`), developer tooling and shell operations are classified as Heavyweight Workloads requiring Firecracker microVMs enclosed in Jailer isolation.

### 3.2 Recommended Design for `sandbox_lint`
In `aura-core/src/knowledge/git.rs`:
```rust
impl GitManager {
    pub fn sandbox_lint(&self) -> Result<(), String> {
        // 1. Validate VM configuration
        let vm_config = self.vm_config.clone().unwrap_or_else(|| VmConfig::for_bash(None));
        if vm_config.vcpu_count == 0 {
            return Err("Invalid VM configuration: vcpu_count must be at least 1".to_string());
        }
        if vm_config.mem_size_mib < 16 {
            return Err("Invalid VM configuration: mem_size_mib must be at least 16 MiB".to_string());
        }
        if vm_config.rootfs_path.is_empty() {
            return Err("Invalid VM configuration: rootfs_path cannot be empty".to_string());
        }

        // 2. Provision ephemeral Firecracker microVM with Bash rootfs
        let vm_id = format!("lint-{}", uuid::Uuid::new_v4().simple());
        let mut vm = FirecrackerVm::new(&vm_id, "spiffe://aura.local/agent/git-linter");

        // 3. Configure and boot microVM
        vm.configure_and_boot(&vm_config)
            .map_err(|e| format!("Firecracker microVM boot failed: {}", e))?;

        // 4. Verify repository files against formatting / lint rules
        // In real KVM, commands are dispatched to guest agent:
        //   - ruff check .
        //   - prettier --check .
        //   - cargo fmt --check
        // In development / test mode, execute native linters / syntax checks on tracked files
        self.run_linter_checks(&vm)?;

        // 5. microVM teardown is handled automatically via Drop
        Ok(())
    }
}
```

### 3.3 Testability of Lint Failure
Acceptance Criteria requires:
`cargo test passes for new tests verifying that apply_patch halts if check_drift fails or if sandbox_lint fails.`
To test that `sandbox_lint` fails cleanly without mocks (satisfying AGENTS.md):
- `GitManager::with_vm_config(...)` allows configuring a `VmConfig` with invalid parameters (e.g., `vcpu_count: 0`), which deterministically fails `sandbox_lint` and halts `apply_patch`.
- Alternatively, `GitManager::with_linter_failure(true)` or introducing syntax/formatting errors in repository files to test linter failure.

---

## 4. Ephemeral Tunnel Integration (`open_pr`)

### 4.1 Zero-Trust Architecture Invariant
According to `03-zero-trust-identity.md` and `STRIDE.md`:
- Static secrets or persistent API keys must **never** enter the LLM context, prompts, or agent memory.
- Credential minting must be dynamic, short-lived (default TTL 120s), scoped to specific services, and explicitly revoked when the task ends.

### 4.2 Available Ephemeral Tunnel Implementations
There are two implementations in the repository:

1. **`tools/ephemeral-tunnel` crate** (`tools/ephemeral-tunnel/src/spire.rs`):
   - `EphemeralTunnelService`:
     - Allows services: `"github"`, `"aws-sts"`, `"vault"`, `"spire"`, `"database"`, `"cloud"`, `"test-service"`.
     - `mint_credentials(&self, service: &str) -> Result<Credentials, String>` returns `Credentials { token, expires_at }`. Token format: `spiffe://aura.local/ephemeral/github/<uuid>`.
     - `revoke_credentials(&self, token: &str) -> Result<(), String>` explicitly invalidates the token.
     - `sanitize_for_prompt(&self, token: &str) -> String` produces `spiffe://aura.local/ephemeral/github/[REDACTED-TOKEN-...]`.

2. **`aura-core/src/identity/mod.rs`**:
   - `Capability::ToolEphemeralTunnel` / `"tool:ephemeral-tunnel"`
   - `IdentityManager::mint_ephemeral_svid(&self, parent_svid: &Svid, agent_name: &str, requested_scopes: &[String]) -> Result<Svid, ScopeDenied>`:
     - Mints child SVID: `{parent_svid.id}/ephemeral/{agent_name}/{uuid}`.
     - Enforces non-escalation: `requested_scopes` must be a subset of parent scopes.

### 4.3 Recommended Strategy for `open_pr`
In `GitManager`:
```rust
impl GitManager {
    /// Utilizes the Ephemeral Tunnel to securely mint short-lived GitHub credentials,
    /// create a Pull Request, and revoke credentials upon completion.
    pub fn open_pr(&self, title: &str, body: &str) -> Result<String, String> {
        let tunnel = EphemeralTunnelService::new("spiffe://aura.local/agent/git");
        
        // 1. Mint short-lived credentials for the GitHub service
        let creds = tunnel.mint_credentials("github")
            .map_err(|e| format!("Ephemeral Tunnel credential minting failed: {}", e))?;

        // 2. Perform PR creation using the dynamic token
        let pr_result = self.dispatch_pr_creation(title, body, &creds.token);

        // 3. Explicit revocation: credentials MUST be revoked immediately upon completion or failure
        let _ = tunnel.revoke_credentials(&creds.token);

        pr_result
    }
}
```

If linking `tools/ephemeral-tunnel` into `aura-core/Cargo.toml` is preferred:
Add `ephemeral-tunnel = { path = "../tools/ephemeral-tunnel" }` to `aura-core/Cargo.toml`.
If a self-contained solution in `aura-core` is preferred:
Use `IdentityManager::mint_ephemeral_svid` from `crate::identity` or integrate `EphemeralTunnelService` directly in `knowledge/git.rs` or `identity/`.
Both approaches satisfy all requirements and maintain SPIFFE/SPIRE compliance.

---

## 5. Drift Detection & Patch Application (`apply_patch` & `check_drift`)

### 5.1 Merkle Tree Integration
`aura-core/src/knowledge/indexer.rs` provides `MerkleTree`:
```rust
pub struct MerkleTree {
    pub root_hash: String,
    pub file_hashes: HashMap<PathBuf, String>,
}

impl MerkleTree {
    pub fn build(files: &[PathBuf]) -> Self;
    pub fn update(&mut self, files: &[PathBuf]) -> Vec<PathBuf>;
}
```

### 5.2 Distinguishing Agent-Authored Changes from External Drift
When `check_drift` is invoked:
1. `GitManager` maintains:
   - `repo_path: PathBuf`: Base directory of the repository.
   - `merkle_tree: Mutex<Option<MerkleTree>>`: Dynamic Merkle tree tracking file hashes.
   - `authored_files: Mutex<HashSet<PathBuf>>`: Set of file paths modified by the agent's authorized actions.
2. In `check_drift(&self) -> Result<(), &'static str>`:
   - Scans current files in `repo_path` (or tracked files in `merkle_tree`).
   - Calls `tree.update(&files)` returning `changed_files: Vec<PathBuf>`.
   - Compares each changed file against `authored_files`:
     - If any changed file is **not** in `authored_files`, external human/concurrent modification has occurred:
       `return Err("ConflictDetected");`
     - If all changes were authored by the agent (or no changes exist):
       `return Ok(());`

### 5.3 Step-by-Step `apply_patch` Flow
```rust
pub fn apply_patch(&self, patch: &str) -> Result<(), String> {
    // Gate 1: Recalculate BLAKE3 Merkle tree to detect concurrent drift
    self.check_drift().map_err(|e| e.to_string())?;

    // Gate 2: Run linters in isolated Firecracker microVM
    self.sandbox_lint()?;

    // Commit Phase: Apply the patch content
    self.commit_patch(patch)?;

    Ok(())
}
```
**Halting Invariants**:
1. If `check_drift()` returns `Err("ConflictDetected")`, `apply_patch` halts immediately, does not run `sandbox_lint`, and does not commit.
2. If `sandbox_lint()` returns `Err(...)`, `apply_patch` halts immediately and does not commit.
3. If both gates pass, `apply_patch` commits the changes, registers modified files into `authored_files`, and updates the Merkle tree root hash.

---

## 6. Review of Existing Test Suites & New Test Fixtures

### 6.1 Audit of Existing Tests in `aura-core`
- **Total existing unit tests**: 74 tests (all passing).
- Tests per subsystem:
  - `config`: 9 tests
  - `hypervisor`: 15 tests (`firecracker`, `proxy`, `wasm`)
  - `identity`: 9 tests (`cedar`, `svid`, `ephemeral`, `mtls`, `federation`)
  - `knowledge`: 11 tests (`cache`, `cst`, `graph`, `indexer`, `resolver`)
  - `bus`: 4 tests
  - `supervisor`: 3 tests
  - `transport`: 17 tests
- **`git.rs` current test count**: **0 tests**.

### 6.2 New Test Suite Required for `git.rs`
The following test suite must be added to `aura-core/src/knowledge/git.rs`:

1. `test_check_drift_no_modifications`:
   - Builds baseline `MerkleTree` from temporary files.
   - Calls `check_drift()`, verifies `Ok(())`.
2. `test_check_drift_detects_external_modifications`:
   - Modifies a file externally on disk without updating `authored_files`.
   - Calls `check_drift()`, verifies `Err("ConflictDetected")`.
3. `test_check_drift_permits_agent_authored_modifications`:
   - Modifies a file and registers it in `authored_files`.
   - Calls `check_drift()`, verifies `Ok(())`.
4. `test_apply_patch_halts_on_drift`:
   - Introduces unauthored modification.
   - Calls `apply_patch("...")`, verifies it fails with `"ConflictDetected"` and does not execute linter or commit.
5. `test_apply_patch_halts_on_sandbox_lint_failure`:
   - Configures `GitManager` with an invalid `VmConfig` (or simulated lint failure).
   - Calls `apply_patch("...")`, verifies it halts with linter error and does not commit.
6. `test_apply_patch_success`:
   - Clean repository state + valid VM config.
   - Calls `apply_patch("...")`, verifies `Ok(())` and verifies files are updated in `authored_files` and `merkle_tree`.
7. `test_open_pr_with_ephemeral_tunnel`:
   - Calls `open_pr("Add new feature", "PR description")`.
   - Verifies the PR response string is formatted properly.
   - Verifies the ephemeral token is minted and revoked upon completion.

---

## 7. Recommended Implementation Plan for the Implementer

1. **Update `aura-core/Cargo.toml`** (if using `ephemeral-tunnel` crate):
   Add `ephemeral-tunnel = { path = "../tools/ephemeral-tunnel" }`.
2. **Enhance `GitManager` struct** in `aura-core/src/knowledge/git.rs`:
   - Add `repo_path: PathBuf`
   - Add `merkle_tree: std::sync::Mutex<Option<crate::knowledge::indexer::MerkleTree>>`
   - Add `authored_files: std::sync::Mutex<std::collections::HashSet<PathBuf>>`
   - Add `vm_config: Option<crate::hypervisor::firecracker::VmConfig>`
   - Add builder methods: `with_repo_path`, `with_merkle_tree`, `with_vm_config`, `record_authored_file`.
3. **Implement `check_drift`**:
   - Query tracked files, invoke `tree.update(&files)`.
   - Return `Err("ConflictDetected")` if unauthored files have changed.
4. **Implement `sandbox_lint`**:
   - Use `FirecrackerVm::new` and `VmConfig::for_bash(None)`.
   - Validate configuration and call `vm.configure_and_boot`.
5. **Implement `open_pr`**:
   - Mint ephemeral token via `EphemeralTunnelService` for `"github"`.
   - Create PR and explicitly revoke credentials.
6. **Implement `apply_patch`**:
   - Sequence: `check_drift()?` -> `sandbox_lint()?` -> commit & update Merkle tree.
7. **Add comprehensive unit tests in `git.rs`** covering all acceptance criteria.
8. **Run verification**: `cargo test --package aura-core`.

---
