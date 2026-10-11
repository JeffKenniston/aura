# Dispatch Assignment — Orchestrator 7

## Objective
Resume and drive the implementation of the 11 partially implemented requirements from the M0-M3 Audit Report for the Aura Multi-Agent Orchestration Operating System to complete production-grade delivery.

## User Mandate
**"Pickup exactly where we left off. The individual quota has reset. Please resume the execution from `.agents/teamwork/orchestrator_6/` or the latest valid state. Milestone 1 code changes were authored; please verify the tests and then proceed to the remaining milestones."**

## Predecessor Context
- Orchestrator 6: `/home/jeff/aura/.agents/teamwork/orchestrator_6/` (`progress.md`, `BRIEFING.md`, `PROJECT.md`, `GATE_STATUS.md`)
- Orchestrator 5: `/home/jeff/aura/.agents/teamwork/orchestrator_5/`
- Worker M1: `/home/jeff/aura/.agents/teamwork/worker_m1_1/` (`progress.md`, `BRIEFING.md`, `DISPATCH.md`)
- Explorers & Surveys:
  - Survey 2 findings: `/home/jeff/aura/.agents/teamwork/explorer_impl_survey_2/handoff.md` (Registry TOML, Bus Dispatch, KVM validation, RootFS immutability)
  - Survey 3 findings: `/home/jeff/aura/.agents/teamwork/explorer_impl_survey_3/handoff.md` (Stateful Bash vsock, CDP vsock forwarder & BrowserTool, GAP-003 docs)
  - Survey 1 notes: `/home/jeff/aura/.agents/teamwork/explorer_impl_survey_1/progress.md`
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
  - Immediate action: Verify `cargo test -p aura-core` (103/103 tests) and `cargo test -p aura-core --test http3_stream_concurrency_test`. Review, challenge, audit gate for M1.
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
- All code must be production-ready: NO mocks, NO stubs, NO TODOs in production paths (AGENTS.md).
- File workspace convention: write only in your assigned directory (`.agents/teamwork/orchestrator_7/`).
- Do NOT repeat the codebase survey from scratch — adopt existing work and proceed immediately with verification and execution.
- Maintain `BRIEFING.md` and `progress.md` in your directory.
- When all acceptance criteria are met and independently verified, report victory to Sentinel.

## 2026-10-11T03:00:45Z
Notice: Server restarted and cargo test hang resolved/verified (110 passed, 0 failed).
Verification Results:
- `cargo test -p aura-core --lib`: 103 passed, 0 failed in 1.14s.
- `cargo test -p aura-core --test http3_stream_concurrency_test`: 2 passed (both 5,000-stream batched and 5,000-stream full concurrency passed with 0 packet loss, 0 stream resets, 0 worker starvation, and 5,170 streams/sec throughput) in 1.36s.
- `cargo test -p aura-core --test m4_empirical_git_stress_test`: 5 passed in 0.08s.
- Full `cargo test -p aura-core`: 110 passed, 0 failed with zero hangs!

Action:
1. Record passing verification for M1 in progress.md and GATE_STATUS.md.
2. Conduct milestone audit gate for M1.
3. Proceed to M2 and subsequent milestones.

