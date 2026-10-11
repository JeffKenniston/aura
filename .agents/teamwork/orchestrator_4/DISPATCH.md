# Dispatch Assignment — Orchestrator 4

## Objective
Resume and drive the implementation of the 11 partially implemented requirements from the M0-M3 Audit Report for the Aura Multi-Agent Orchestration Operating System to complete production-grade delivery.

## User Mandate
**"Don't restart from the top. Pickup where you left off. Please resume the state, execution plan, and milestone decomposition from the previous orchestrator run located in `.agents/teamwork/orchestrator_3/` instead of surveying the codebase again from scratch."**

## Predecessor Context
- Predecessor working directory: `/home/jeff/aura/.agents/teamwork/orchestrator_3/`
- Predecessor briefing: `/home/jeff/aura/.agents/teamwork/orchestrator_3/BRIEFING.md`
- Predecessor progress: `/home/jeff/aura/.agents/teamwork/orchestrator_3/progress.md`
- Survey 2 findings: `/home/jeff/aura/.agents/teamwork/explorer_impl_survey_2/handoff.md` (Registry TOML, Bus Dispatch, KVM validation, RootFS immutability)
- Survey 3 findings: `/home/jeff/aura/.agents/teamwork/explorer_impl_survey_3/handoff.md` (Stateful Bash vsock, CDP vsock forwarder & BrowserTool, GAP-003 docs)
- Survey 1 notes: `/home/jeff/aura/.agents/teamwork/explorer_impl_survey_1/progress.md` (Runtime & Concurrency, QUIC 5k load test, MCP HTTP/3 baseline verified)

## Milestone Decomposition (per orchestrator_3 plan)
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
  - Claim completion to Sentinel for Victory Audit

## Constraints & Rules
- You are a pure orchestrator. NEVER write code yourself. Dispatch workers and specialists.
- All code must be production-ready: NO mocks, NO stubs, NO TODOs in production paths.
- File workspace convention: write only in your assigned directory (`.agents/teamwork/orchestrator_4/`).


## 2026-10-10T05:39:22Z
You are the Project Orchestrator for the Aura project.
Your assigned working directory is: /home/jeff/aura/.agents/teamwork/orchestrator_4/
Read your dispatch assignment at: /home/jeff/aura/.agents/teamwork/orchestrator_4/DISPATCH.md

CRITICAL USER MANDATE:
"Don't restart from the top. Pickup where you left off. Please resume the state, execution plan, and milestone decomposition from the previous orchestrator run located in `.agents/teamwork/orchestrator_3/` instead of surveying the codebase again from scratch."

Context and Previous Artifacts:
- Predecessor working directory: /home/jeff/aura/.agents/teamwork/orchestrator_3/
- Predecessor briefing: /home/jeff/aura/.agents/teamwork/orchestrator_3/BRIEFING.md
- Predecessor progress: /home/jeff/aura/.agents/teamwork/orchestrator_3/progress.md
- Completed Explorer 2 Handoff: /home/jeff/aura/.agents/teamwork/explorer_impl_survey_2/handoff.md (Registry TOML, Bus Dispatch, KVM validation, RootFS immutability)
- Completed Explorer 3 Handoff: /home/jeff/aura/.agents/teamwork/explorer_impl_survey_3/handoff.md (Stateful Bash vsock, CDP vsock forwarder & BrowserTool, GAP-003 docs)
- Explorer 1 Progress & verified baseline: /home/jeff/aura/.agents/teamwork/explorer_impl_survey_1/progress.md
- Authoritative user request: /home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md

Proceed directly into executing the milestones (M1 -> M2 -> M3 -> M4 -> M5) by dispatching workers, reviewers, and verification agents.
Maintain /home/jeff/aura/.agents/teamwork/orchestrator_4/BRIEFING.md and /home/jeff/aura/.agents/teamwork/orchestrator_4/progress.md.
When all acceptance criteria are met and independently verified, report victory to Sentinel.
