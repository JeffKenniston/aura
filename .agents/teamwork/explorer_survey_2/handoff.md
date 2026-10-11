# Handoff Report: Survey Explorer 2 (Firecracker Sandbox Lint, Ephemeral Tunnel & Apply Patch)

## 1. Observation
1. **Firecracker Module Location and API**:
   - `aura-core/src/hypervisor/firecracker.rs:18-28`: `SpecializedRootfs` enum (`Bash`, `Browser`, `Computer`, `Custom`).
   - `aura-core/src/hypervisor/firecracker.rs:87-135`: `VmConfig` struct with constructor `VmConfig::for_bash(base_dir: Option<&str>)` defaulting to `bash.ext4`, 1 vCPU, 128 MiB RAM.
   - `aura-core/src/hypervisor/firecracker.rs:332-536`: `FirecrackerVm` struct with methods `new(id, svid)`, `with_config(id, svid, config)`, `configure_and_boot(&vm_config)`, and `teardown()`.
   - `aura-core/src/hypervisor/firecracker.rs:504-508`: `send_api_request` checks `if !self.socket_path.exists() { return Ok(()); }`, allowing tests without real KVM jailer to execute gracefully.
2. **Current `git.rs` Implementation**:
   - `aura-core/src/knowledge/git.rs:1-43`: Contains stubbed `GitManager` with `check_drift(&self) -> Result<(), &'static str>` returning `Ok(())`, `sandbox_lint(&self) -> Result<(), String>` returning `Ok(())`, `apply_patch(&self, _patch: &str) -> Result<(), String>` calling both checks without committing, and `open_pr(&self, title: &str, _body: &str) -> Result<String, String>` returning dummy format string. Currently 0 tests exist in `git.rs`.
3. **Ephemeral Tunnel Implementations**:
   - `tools/ephemeral-tunnel/src/spire.rs:14-172`: `EphemeralTunnelService` with `mint_credentials("github") -> Result<Credentials, String>`, `revoke_credentials(&token)`, and `sanitize_for_prompt(&token)`.
   - `aura-core/src/identity/mod.rs:565-600`: `IdentityManager::mint_ephemeral_svid(&parent_svid, agent_name, requested_scopes)` minting child SVID `{parent}/ephemeral/{agent}/{uuid}` with scope non-escalation.
4. **Merkle Tree Drift Engine**:
   - `aura-core/src/knowledge/indexer.rs:13-62`: `MerkleTree` with `build(files: &[PathBuf]) -> Self` and `update(&mut self, files: &[PathBuf]) -> Vec<PathBuf>` returning paths of modified files using BLAKE3 hashes.
5. **Existing Test Suite Status**:
   - Ran `cargo test --package aura-core`: Passed 74 tests in 1.16s (0 failed).
   - Ran `cargo test --package ephemeral-tunnel`: Passed 5 tests.
   - Ran `cargo test --package ast-blast-radius`: Passed 6 tests.

## 2. Logic Chain
1. From Observation 1, `FirecrackerVm` and `VmConfig::for_bash` are fully implemented and ready in `aura-core/src/hypervisor/firecracker.rs`. Requirement R3 prohibits running linters directly on the host. Therefore, `sandbox_lint` in `git.rs` must instantiate `FirecrackerVm::new` and boot using `VmConfig::for_bash(None)`.
2. From Observation 1 (`firecracker.rs:504-508`), unit tests do not require real KVM or jailer daemon because non-existent sockets exit `send_api_request` gracefully. However, invalid `VmConfig` (e.g. `vcpu_count: 0`, empty paths, or `< 16` MiB RAM) can be rejected before or during configuration, enabling deterministic testing that `apply_patch` halts on lint failure without using mocks (respecting `AGENTS.md`).
3. From Observation 3, `EphemeralTunnelService` supports the `"github"` service and satisfies Zero-Trust invariants (IAM-003, IAM-004, COG-009) by minting short-lived tokens and explicitly revoking them. Therefore, `open_pr` can use this workflow to authenticate PR creation and immediately revoke the token.
4. From Observation 4, `MerkleTree::update(&files)` returns the list of modified files. By keeping an `authored_files: Mutex<HashSet<PathBuf>>` in `GitManager`, `check_drift` can distinguish between changes authored by the agent and concurrent external modifications. Any modified file not in `authored_files` triggers `Err("ConflictDetected")`.
5. From Observation 2, `apply_patch` currently has the skeleton `self.check_drift()?; self.sandbox_lint()?;`. Integrating the concrete drift detection and sandbox linting ensures `apply_patch` halts if either pre-condition fails, and only commits and updates the Merkle tree when both succeed.

## 3. Caveats
- `aura-core/Cargo.toml` does not currently include `ephemeral-tunnel` as a dependency. The implementer can either add `ephemeral-tunnel = { path = "../tools/ephemeral-tunnel" }` or use `IdentityManager::mint_ephemeral_svid` directly in `aura-core`. Both alternatives have been fully documented in `survey_report.md`.
- No actual Git binary or `.git` directory manipulation is required for unit tests; tracking via `MerkleTree` and file writes in a temporary directory matches the existing pattern in `aura-core`.

## 4. Conclusion
The implementation path for `git.rs` is clear, fully supported by existing infrastructure in `hypervisor/firecracker.rs`, `indexer.rs`, and `identity/ephemeral-tunnel`, and can be implemented with zero mocks while passing all verification gates. Detailed findings and type signatures are documented in `/home/jeff/aura/.agents/teamwork/explorer_survey_2/survey_report.md`.

## 5. Verification Method
1. Verify existing tests pass:
   ```bash
   cargo test --package aura-core
   cargo test --package ephemeral-tunnel --manifest-path tools/Cargo.toml
   ```
2. Inspect findings report:
   ```bash
   cat /home/jeff/aura/.agents/teamwork/explorer_survey_2/survey_report.md
   ```
3. Verification condition for the upcoming implementation:
   `cargo test --package aura-core` must pass all existing 74 tests plus new tests in `aura-core/src/knowledge/git.rs` for `check_drift`, `sandbox_lint`, `open_pr`, and `apply_patch` halting behavior.
