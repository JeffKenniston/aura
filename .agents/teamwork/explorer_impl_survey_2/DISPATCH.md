## 2026-10-10T04:50:47Z
You are explorer_impl_survey_2, a read-only exploration agent.
Your working directory is: `/home/jeff/aura/.agents/teamwork/explorer_impl_survey_2/`

You MUST read `/home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md` (see the section starting at `## 2026-10-10T04:46:40Z`), `/home/jeff/aura/M0_M3_Audit_Report.md`, and `/home/jeff/aura/requirements.md`.

Your objective:
Conduct an in-depth codebase survey for:
1. HOST-011: Declarative TOML manifest loading for Component Registry via Serde in `aura-core/src/runtime/registry.rs`, typed `ToolManifestFile` with WASM and Firecracker entries, and integration tests with a valid TOML file.
2. BUS-003: Replace simulated task execution in Zenoh bus listener (`aura-core/src/bus/mod.rs`) with real WASM runtime execution and Firecracker microVM dispatch using dynamic SVID extraction (replacing hardcoded `mock_spiffe_id` and synthetic result strings).
3. EXE-VM-002: Nested virtualization host `/dev/kvm` validation (`validate_kvm_support()`), checking file existence, read/write permissions, error variants (`KvmDeviceNotFound`, `KvmPermissionDenied`), CI skip override (`AURA_SKIP_KVM_VALIDATION`), and unit tests.
4. EXE-VM-011: Immutable rootfs layering (`"is_read_only": true`) and ephemeral scratch disk / overlayfs attachment in Firecracker microVM, ensuring golden disk images on the host are not mutated across boots.

Analyze existing code:
- `aura-core/src/runtime/registry.rs`
- `aura-core/src/bus/`
- `aura-core/src/hypervisor/`

Scope boundaries:
- DO NOT modify any code or files outside your working directory.
- Write your findings, proposed structs/APIs, exact files to edit, test design, and step-by-step implementation plan to `handoff.md` in `/home/jeff/aura/.agents/teamwork/explorer_impl_survey_2/handoff.md`.
- Update `progress.md` in your directory as you work.
- When finished, use `send_message` to notify caller (c402a87d-8780-44d1-b1d0-cba4238c7a72).
