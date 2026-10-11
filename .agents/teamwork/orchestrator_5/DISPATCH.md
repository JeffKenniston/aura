# Dispatch Assignment — Orchestrator 5

## Objective
Resume and drive the implementation of the 11 partially implemented requirements from the M0-M3 Audit Report for the Aura Multi-Agent Orchestration Operating System to complete production-grade delivery.

## User Mandate
**"Pickup exactly where you left off. Please resume the state, execution plan, and milestone decomposition from the latest orchestrator run (likely `.agents/teamwork/orchestrator_4/` or `.agents/teamwork/orchestrator_3/`) instead of starting over."**

## Predecessor Context
- Latest orchestrator runs:
  - `/home/jeff/aura/.agents/teamwork/orchestrator_4/` (`progress.md`, `BRIEFING.md`, `DISPATCH.md`)
  - `/home/jeff/aura/.agents/teamwork/orchestrator_3/` (`progress.md`, `BRIEFING.md`, `DISPATCH.md`)
- Survey 2 findings: `/home/jeff/aura/.agents/teamwork/explorer_impl_survey_2/handoff.md` (Registry TOML, Bus Dispatch, KVM validation, RootFS immutability)
- Survey 3 findings: `/home/jeff/aura/.agents/teamwork/explorer_impl_survey_3/handoff.md` (Stateful Bash vsock, CDP vsock forwarder & BrowserTool, GAP-003 docs)
- Survey 1 notes: `/home/jeff/aura/.agents/teamwork/explorer_impl_survey_1/progress.md` (Runtime & Concurrency, QUIC 5k load test, MCP HTTP/3 baseline verified)
- Master baseline and audit:
  - `/home/jeff/aura/M0_M3_Audit_Report.md`
  - `/home/jeff/aura/requirements.md`
  - `/home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md`

## Milestone Decomposition (Adopted from orchestrator_3 & orchestrator_4)
- **M1: Runtime & Concurrency** (M0-SPIKE-03, HOST-002, HOST-004)
  - Dedicated multi-threaded Tokio runtime pool with separate blocking task pool
  - Runtime health metrics (`RuntimeMetrics` via `tokio_unstable`)
  - Empirical load test asserting 5,000+ concurrent QUIC streams without starvation (`http3_stream_concurrency_test.rs`)
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
- File workspace convention: write only in your assigned directory (`.agents/teamwork/orchestrator_5/`).
- Do NOT repeat the codebase survey from scratch — adopt the existing survey findings and proceed directly to Milestone 1 execution.
- Maintain `BRIEFING.md` and `progress.md` in your directory.
- When all acceptance criteria are met and independently verified, report victory to Sentinel.


## 2026-10-10T05:43:51Z
You are the Project Orchestrator for the Aura Multi-Agent Orchestration Operating System.
Your assigned working directory is: /home/jeff/aura/.agents/teamwork/orchestrator_5/

CRITICAL USER MANDATE:
"Pickup exactly where you left off. Please resume the state, execution plan, and milestone decomposition from the latest orchestrator run (likely `.agents/teamwork/orchestrator_4/` or `.agents/teamwork/orchestrator_3/`) instead of starting over."

Please read your dispatch instructions at:
/home/jeff/aura/.agents/teamwork/orchestrator_5/DISPATCH.md

Key predecessor context already completed:
- /home/jeff/aura/.agents/teamwork/orchestrator_4/progress.md & BRIEFING.md
- /home/jeff/aura/.agents/teamwork/orchestrator_3/progress.md & BRIEFING.md
- /home/jeff/aura/.agents/teamwork/explorer_impl_survey_2/handoff.md (Detailed findings and designs for Registry TOML, Bus Dispatch, KVM validation, RootFS immutability)
- /home/jeff/aura/.agents/teamwork/explorer_impl_survey_3/handoff.md (Detailed findings and designs for Stateful Bash vsock, CDP vsock forwarder & BrowserTool, GAP-003 docs)
- /home/jeff/aura/.agents/teamwork/explorer_impl_survey_1/progress.md (Runtime & Concurrency baseline)
- /home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md

Do NOT repeat the survey phase. Adopt the 5-milestone plan:
1. Milestone 1: Runtime & Concurrency (M0-SPIKE-03, HOST-002, HOST-004)
2. Milestone 2: Declarative Config & MCP HTTP/3 (HOST-011, NET-007)
3. Milestone 3: Authentic Task Execution & MicroVM Sandboxing (BUS-003, EXE-VM-002, EXE-VM-011)
4. Milestone 4: Persistent Stateful Tools & GAP-003 (TOOL-BASH-002, TOOL-BRW-003, GAP-003)
5. Milestone 5: Final Multi-Agent Verification Gate & Delivery

Initialize your BRIEFING.md and progress.md in /home/jeff/aura/.agents/teamwork/orchestrator_5/ and proceed immediately with Milestone 1 dispatch. Keep your progress.md updated regularly as a heartbeat.
When all acceptance criteria are fully met and independently verified, report victory to Sentinel.
