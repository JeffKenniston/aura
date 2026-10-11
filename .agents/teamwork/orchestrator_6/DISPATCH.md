# Dispatch Assignment — Orchestrator 6

## Objective
Resume and drive the implementation of the 11 partially implemented requirements from the M0-M3 Audit Report for the Aura Multi-Agent Orchestration Operating System to complete production-grade delivery.

## User Mandate
**"Pickup exactly where we left off. The individual quota has reset. Please resume the execution from `.agents/teamwork/orchestrator_5/` or `.agents/teamwork/worker_m1_1/`. Milestone 1 code changes were already authored; please verify the 103/103 tests and then proceed to the remaining milestones."**

## Predecessor Context
- Orchestrator 5: `/home/jeff/aura/.agents/teamwork/orchestrator_5/` (`progress.md`, `BRIEFING.md`, `PROJECT.md`, `GATE_STATUS.md`)
- Worker M1: `/home/jeff/aura/.agents/teamwork/worker_m1_1/` (`progress.md`, `BRIEFING.md`, `DISPATCH.md`)
- Earlier Orchestrators:
  - `/home/jeff/aura/.agents/teamwork/orchestrator_4/` (`progress.md`, `BRIEFING.md`, `DISPATCH.md`)
  - `/home/jeff/aura/.agents/teamwork/orchestrator_3/` (`progress.md`, `BRIEFING.md`, `DISPATCH.md`)
- Explorers & Surveys:
  - Survey 2 findings: `/home/jeff/aura/.agents/teamwork/explorer_impl_survey_2/handoff.md` (Registry TOML, Bus Dispatch, KVM validation, RootFS immutability)
  - Survey 3 findings: `/home/jeff/aura/.agents/teamwork/explorer_impl_survey_3/handoff.md` (Stateful Bash vsock, CDP vsock forwarder & BrowserTool, GAP-003 docs)
  - Survey 1 notes: `/home/jeff/aura/.agents/teamwork/explorer_impl_survey_1/progress.md`
  - M1 explorers: `explorer_m1_1/`, `explorer_m1_2/`, `explorer_m1_3/`
- Master baseline and audit:
  - `/home/jeff/aura/M0_M3_Audit_Report.md`
  - `/home/jeff/aura/requirements.md`
  - `/home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md`

## Milestone Decomposition
- **M1: Runtime & Concurrency** (M0-SPIKE-03, HOST-002, HOST-004)
  - Code authored by worker_m1_1:
    - Dedicated multi-threaded Tokio runtime pool with separate blocking task pool (`aura-core/src/runtime/mod.rs`)
    - Runtime health metrics (`RuntimeMetrics` via `tokio_unstable`, `.cargo/config.toml`)
    - Empirical load test asserting 5,000+ concurrent QUIC streams (`aura-core/tests/http3_stream_concurrency_test.rs`)
    - Runtime config in `aura-core/src/config/mod.rs`, `aura-core/src/lib.rs`, `aura-cli/src/main.rs`, `aura-core/src/transport/http3.rs`
  - Immediate action: Verify `cargo test -p aura-core` (103/103 tests) and `cargo test -p aura-core --test http3_stream_concurrency_test`.
- **M2: Declarative Config & MCP HTTP/3** (HOST-011, NET-007)
  - TOML manifest loading for Component Registry via Serde
  - Registry load tests parsing valid TOML for WASM and Firecracker backends
  - Stateless MCP server over HTTP/3 handling JSON-RPC 2.0 `tools/list` and `tools/call`
- **M3: Authentic Task Execution & MicroVM Sandboxing** (BUS-003, EXE-VM-002, EXE-VM-011)
  - Replace simulated task execution in Zenoh bus listener with real WASM and Firecracker microVM dispatch
  - Dynamic SVID integration
  - `/dev/kvm` nested virtualization validation and error propagation tests
  - Immutable guest rootfs layering (overlayfs / scratch disks) preventing golden image mutation
- **M4: Persistent Stateful Tools & GAP-003** (TOOL-BASH-002, TOOL-BRW-003, GAP-003)
  - Persistent AF_VSOCK shell daemon for bash session state continuity (env vars & cwd across calls)
  - Host-to-guest vsock TCP forwarder for Python CDP client to Chromium (port 9222)
  - Maintain static WIT linking mitigation for WASI 0.3 dynamic linking limitation (GAP-003) with architecture documentation
- **M5: Final Multi-Agent Verification Gate & Delivery**
  - Run full test suite (`cargo test -p aura-core`, `pytest aura-sdk/tests`, new integration tests)
  - Reviewer and Challenger verification
  - Forensic Auditor verification
  - Claim completion to Sentinel for Victory Audit

## Constraints & Rules
- You are a pure orchestrator. NEVER write code yourself. Dispatch workers and specialists.
- All code must be production-ready: NO mocks, NO stubs, NO TODOs in production paths.
- File workspace convention: write only in your assigned directory (`.agents/teamwork/orchestrator_6/`).
- Do NOT repeat the codebase survey from scratch — adopt existing work and proceed immediately with verification and execution.
- Maintain `BRIEFING.md` and `progress.md` in your directory.
- When all acceptance criteria are met and independently verified, report victory to Sentinel.

## 2026-10-10T23:15:43Z
[Message] timestamp=2026-10-10T23:15:43Z sender=a28098ca-6a3c-4925-9b71-09e7147426c0 priority=MESSAGE_PRIORITY_HIGH content=You are the Project Orchestrator for the Aura Multi-Agent Orchestration Operating System.
Your working directory is: /home/jeff/aura/.agents/teamwork/orchestrator_6/

CRITICAL MANDATE:
"Pickup exactly where we left off. The individual quota has reset. Please resume the execution from `.agents/teamwork/orchestrator_5/` or `.agents/teamwork/worker_m1_1/`. Milestone 1 code changes were already authored; please verify the 103/103 tests and then proceed to the remaining milestones."

Please read:
- Assignment: `/home/jeff/aura/.agents/teamwork/orchestrator_6/DISPATCH.md`
- Authoritative requests: `/home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md`
- Predecessor progress and state:
  - `/home/jeff/aura/.agents/teamwork/orchestrator_5/progress.md`
  - `/home/jeff/aura/.agents/teamwork/orchestrator_5/PROJECT.md`
  - `/home/jeff/aura/.agents/teamwork/worker_m1_1/progress.md`
  - `/home/jeff/aura/.agents/teamwork/explorer_impl_survey_2/handoff.md`
  - `/home/jeff/aura/.agents/teamwork/explorer_impl_survey_3/handoff.md`
  - Reference audit: `/home/jeff/aura/M0_M3_Audit_Report.md` and `/home/jeff/aura/requirements.md`

Rules:
- You are a pure orchestrator. NEVER write code directly. Dispatch subagents for implementations, tests, and reviews.
- Zero mock / stub policy per AGENTS.md.
- Maintain `BRIEFING.md` and `progress.md` in `/home/jeff/aura/.agents/teamwork/orchestrator_6/`.
- Verify Milestone 1 (103/103 tests and QUIC 5k load test), then execute Milestones 2, 3, 4, and 5.
- When all acceptance criteria are met and independently verified, report completion back to Sentinel.

