# Dispatch for Worker M3 Implementation

You are `worker_m3_1` (role: `teamwork_preview_worker`).
Your assigned working directory is `/home/jeff/aura/.agents/teamwork/worker_m3_1/`.

## Mandatory Reading:
- User Request: `/home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md` (R3)
- Master Baseline: `/home/jeff/aura/M0_M3_Audit_Report.md` (§ BUS-003, EXE-VM-002, EXE-VM-011)
- Scope: `/home/jeff/aura/.agents/teamwork/orchestrator_7/PROJECT.md`
- Implementation Survey: `/home/jeff/aura/.agents/teamwork/explorer_impl_survey_2/handoff.md` (§ 1.2, 1.3, 1.4, 2.2, 2.3, 2.4, 3.2, 3.3, 3.4)
- Repository Constitution: `/home/jeff/aura/AGENTS.md` (Strict zero mock/stub policy)
- Domain Skills:
  - `/home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-bus-domain/SKILL.md`
  - `/home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-exe-vm-domain/SKILL.md`

## Milestone 3 Tasks:
### 1. BUS-003: Authentic Zenoh Task Dispatch with Dynamic SVID & Real Execution
Target File: `aura-core/src/bus/mod.rs`
- Enhance `TaskPayload` with optional fields:
  - `payload: Option<serde_json::Value>`
  - `svid: Option<String>`
  - `fuel_budget: Option<u64>`
- Enhance `TaskResult` with execution telemetry:
  - `stdout: String`, `stderr: String`, `exit_code: i32`, `duration_ms: u64`
  - Preserve `result: String` for backward compatibility.
- In `start_task_listener`:
  - Dynamically extract SVID: from `task.svid`, `task.agent` (if SPIFFE formatted), or topic key expression prefix.
  - Enforce capability validation via `CapabilityEnforcer::validate_svid(&svid_str)` (replace synthetic `"mock_jwt"`).
  - Enforce rate limiting via `limiter.check_and_consume(&svid_str)` (replace synthetic `"spiffe://aura.local/agent/1"`).
  - Replace the synthetic string formatting (`format!("Executed {} in WASM", task.instruction)`) with real execution dispatch:
    - Route `task.instruction` through `Registry` or execute via `hypervisor::wasm` (for WASM components) or `FirecrackerVm` (for Firecracker).
    - Capture real stdout, stderr, exit code, and execution duration.
  - Update `test_task_dispatch_and_completion` to verify real execution output and fields.

### 2. EXE-VM-002: Host `/dev/kvm` Nested Virtualization Validation & Error Propagation
Target File: `aura-core/src/hypervisor/firecracker.rs` & `aura-core/src/hypervisor/mod.rs`
- Implement `pub fn validate_kvm_support() -> Result<(), KvmError>`:
  - Check existence and read/write opening of `/dev/kvm`.
  - Distinguish between `KvmDeviceNotFound` (file doesn't exist) and `KvmPermissionDenied` (permission error).
  - Support `AURA_SKIP_KVM_VALIDATION=1` environment variable override for containerized/CI environments.
  - Integrate `validate_kvm_support()` into `hypervisor::init_firecracker()`.
  - Add unit tests verifying:
    - Error propagation when `/dev/kvm` is missing or inaccessible.
    - Behavior when `AURA_SKIP_KVM_VALIDATION=1` is set.

### 3. EXE-VM-011: Guest Filesystem Layering & Base Image Immutability
Target File: `aura-core/src/hypervisor/firecracker.rs`
- In `VmConfig`: add `is_read_only_rootfs: bool` (default true) and `scratch_disk_path: Option<String>`.
- In `FirecrackerVm::configure_and_boot()`:
  - Configure root block device PUT `/drives/rootfs` with `"is_read_only": true`.
  - Set kernel boot args to include `ro root=/dev/vda`.
  - If `scratch_disk_path` is present, configure secondary drive PUT `/drives/scratch` (read-write on `/dev/vdb`).
  - In `teardown()`, clean up any ephemeral scratch disk image files.
  - Add unit tests asserting `"is_read_only": true` on the rootfs drive JSON payload.

### 4. Build & Test Verification:
- Run `cargo check --all-targets`
- Run `cargo test -p aura-core` (all tests must pass with 0 failures)
- Ensure zero warnings, zero mocks, zero stubs in production code.

Write complete results and test logs in `/home/jeff/aura/.agents/teamwork/worker_m3_1/handoff.md`.

## Mandatory Integrity Warning:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.


## 2026-10-11T03:36:27Z
Received message from orchestrator:
You are worker_m3_1 (role: teamwork_preview_worker).
Your working directory is `/home/jeff/aura/.agents/teamwork/worker_m3_1/`.
Read your task assignment in `/home/jeff/aura/.agents/teamwork/worker_m3_1/DISPATCH.md`.
