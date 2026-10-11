# Dispatch Assignment — Orchestrator 8

## Objective
Resume and drive the implementation of the 11 partially implemented requirements from the M0-M3 Audit Report for the Aura Multi-Agent Orchestration Operating System to complete production-grade delivery.

## User Mandate
**"Pickup exactly where we left off. The individual quota has reset. Please resume the execution from `.agents/teamwork/orchestrator_7/` or the latest valid state."**

## Predecessor Context & Verified Progress
- **Milestone 1**: **PASSED** (Gate Closed)
  - Dedicated multi-threaded Tokio runtime pool, RuntimeMetrics, and empirical 5,000 QUIC stream load test (`http3_stream_concurrency_test.rs`).
  - 110/110 tests pass in `aura-core`.
  - Independent forensic audit: `CLEAN` verdict from `auditor_m1_2`.
- **Milestone 2**: **PASSED** (Gate Closed)
  - Declarative TOML manifest loading for Component Registry via Serde (`HOST-011` in `aura-core/src/runtime/registry.rs`).
  - Stateless JSON-RPC 2.0 MCP server over HTTP/3 (`NET-007` in `aura-core/src/transport/mcp.rs`).
  - 118/118 tests pass in `aura-core`.
  - Independent forensic audit: `CLEAN` verdict from `auditor_m2_1`.
- **Milestone 3**: **IMPLEMENTATION COMPLETE** (128/128 tests pass)
  - `BUS-003`: Dynamic SPIFFE SVID extraction, Zero-Trust capability validation, rate limiting, and authentic WASM/Firecracker task dispatch in `aura-core/src/bus/mod.rs`.
  - `EXE-VM-002`: `/dev/kvm` hardware availability validation with `AURA_SKIP_KVM_VALIDATION` bypass and error propagation in `aura-core/src/hypervisor/firecracker.rs` and `aura-core/src/hypervisor/mod.rs`.
  - `EXE-VM-011`: MicroVM immutable rootfs (`is_read_only: true`, `ro root=/dev/vda`) and ephemeral scratch disk layering in `aura-core/src/hypervisor/firecracker.rs`.
  - Report: `/home/jeff/aura/.agents/teamwork/worker_m3_1/handoff.md`.
  - **Immediate next action**: Dispatch auditor (`auditor_m3_2`) to formalize Milestone 3 audit gate (CLEAN verdict).
- **Milestone 4**: **READY FOR IMPLEMENTATION**
  - `TOOL-BASH-002`: Persistent AF_VSOCK shell daemon for bash session state continuity (env vars & cwd across sequential calls).
  - `TOOL-BRW-003`: Host-to-guest vsock TCP proxy connecting Chromium CDP port 9222 and async Python CDP client (`BrowserTool`).
  - `GAP-003`: Architectural documentation of WASI 0.3 dynamic linking limitation & static WIT linking mitigation in `aura-core/src/hypervisor/wasm.rs`, `docs/architecture/adrs/ADR-001-Execution-Host.md`, and `references/02-execution-sandboxes.md`.
- **Milestone 5**: **FINAL MULTI-AGENT VERIFICATION GATE & DELIVERY**
  - Run full test suite (`cargo test -p aura-core`, `pytest aura-sdk/tests`, integration tests).
  - Victory Challenger verification.
  - Victory Forensic Auditor verification.
  - Report victory to Sentinel.

## Constraints & Rules
- Pure orchestrator: NEVER write production code directly. Dispatch workers and specialists.
- Zero mock / stub policy per `AGENTS.md`. Code must be production-ready.
- Write only to your working directory (`.agents/teamwork/orchestrator_8/`).
- Maintain `BRIEFING.md`, `progress.md`, and `GATE_STATUS.md`.
- Report victory to Sentinel via `send_message` when all acceptance criteria are met and independently verified.
