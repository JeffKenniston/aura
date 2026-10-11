# BRIEFING — 2026-10-10T04:58:30Z

## Mission
Conduct in-depth codebase survey for HOST-011, BUS-003, EXE-VM-002, and EXE-VM-011 in aura-core, producing a comprehensive handoff report.

## 🔒 My Identity
- Archetype: explorer
- Roles: codebase investigation, synthesis
- Working directory: /home/jeff/aura/.agents/teamwork/explorer_impl_survey_2/
- Original parent: c402a87d-8780-44d1-b1d0-cba4238c7a72
- Milestone: M0-M3 gap implementation survey

## 🔒 Key Constraints
- Read-only investigation — do NOT implement or modify any code or files outside working directory
- Production-ready standards: ABSOLUTELY NO mocks, stubs, or TODOs in proposals
- Detailed structs/APIs, exact files to edit, test design, and step-by-step implementation plan

## Current Parent
- Conversation ID: c402a87d-8780-44d1-b1d0-cba4238c7a72
- Updated: not yet

## Investigation State
- **Explored paths**:
  - `aura-core/src/runtime/registry.rs`
  - `aura-core/src/bus/mod.rs`
  - `aura-core/src/hypervisor/firecracker.rs`
  - `aura-core/src/hypervisor/mod.rs`
  - `aura-core/src/hypervisor/wasm.rs`
  - `aura-core/src/identity/mod.rs`
  - `aura-core/src/knowledge/git.rs`
  - `aura-core/src/lib.rs`
- **Key findings**:
  - HOST-011: `registry.rs:33-43` is mocked with a simple `contains("test")` branch returning `/tmp/mock.wasm`. Serde `ToolManifestFile` with enum `ToolEntryConfig` (Wasm / Firecracker) is cleanly designable with `toml::from_str`.
  - BUS-003: `bus/mod.rs:145-161` uses synthetic result strings and `start_router` hardcodes `mock_spiffe_id = "spiffe://aura.local/agent/1"` and `CapabilityEnforcer::validate_svid("mock_jwt")`. Needs dynamic SVID extraction (via payload, agent SPIFFE ID, or Zenoh key expression), capability check, and authentic execution routing.
  - EXE-VM-002: Host `/dev/kvm` currently exists with permissions `crw-rw---- 1 root kvm`. Non-root user `jeff` is not in `kvm` group, so direct open yields `PermissionDenied`. `validate_kvm_support()` must check existence, permissions, return typed `KvmError`, and respect `AURA_SKIP_KVM_VALIDATION=1`.
  - EXE-VM-011: `firecracker.rs:444` mounts root drive with `"is_read_only": false`. Needs `"is_read_only": true`, secondary scratch disk (`/drives/scratch`), and golden image hash preservation across microVM boots.
- **Unexplored areas**: None. All 4 target areas thoroughly surveyed.

## Key Decisions Made
- Architected production-grade designs for all 4 requirements adhering to Zero-Trust SPIFFE and Docker-Free sandboxing invariants.
- Synthesized full 5-component handoff report.

## Artifact Index
- DISPATCH.md — incoming instructions log
- BRIEFING.md — persistent working memory
- progress.md — liveness heartbeat and progress tracking
- handoff.md — 5-component handoff report
