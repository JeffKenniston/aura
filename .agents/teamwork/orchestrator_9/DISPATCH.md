# Dispatch Assignment — Orchestrator 9

## Objective
Resume and drive the implementation of Milestone 4 Iteration 2 (Persistent Stateful Tools & GAP-003) and Milestone 5 (Final Multi-Agent Verification Gate & Delivery) from the M0-M3 Audit Report for the Aura Multi-Agent Orchestration Operating System to complete production-grade delivery.

## User Mandate
**"Pickup right where Orchestrator 8 left off.
Milestones 1, 2, 3 have been completed and verified (PASS in GATE_STATUS.md).
Milestone 4 (Persistent Stateful Tools & GAP-003) Iteration 1 failed due to defects identified by challengers and auditor.
Iteration 2 fix exploration was conducted by explorer_m4_fix_1, explorer_m4_fix_2, and explorer_m4_fix_3:
- Drop-in fix for `aura-sdk/aura_sdk/tools/bash.py` is ready in `/home/jeff/aura/.agents/teamwork/explorer_m4_fix_1/proposed_bash.py`
- Drop-in fix for `aura-sdk/aura_sdk/tools/browser.py` is ready in `/home/jeff/aura/.agents/teamwork/explorer_m4_fix_2/proposed_browser.py`
- Test reconciliation patch for `aura-sdk/tests/test_browser_tool_stress.py` is in `/home/jeff/aura/.agents/teamwork/explorer_m4_fix_2/test_stress_remediation.patch`
- Fix cwd quoting and per-session concurrency in `aura-core/src/hypervisor/firecracker.rs`

Next Steps:
1. Apply the fixes to `bash.py`, `browser.py`, `test_browser_tool_stress.py`, and `firecracker.rs`.
2. Verify with `pytest aura-sdk/tests` (84/84 passing) and `cargo test -p aura-core`.
3. Complete Gate 4 Iteration 2 verification.
4. Execute Milestone 5 final workspace verification and deliver completion report."**

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
- **Milestone 3**: **PASSED** (Gate Closed)
  - `BUS-003`: Dynamic SPIFFE SVID extraction, Zero-Trust capability validation, rate limiting, and authentic WASM/Firecracker task dispatch in `aura-core/src/bus/mod.rs`.
  - `EXE-VM-002`: `/dev/kvm` hardware availability validation with `AURA_SKIP_KVM_VALIDATION` bypass and error propagation in `aura-core/src/hypervisor/firecracker.rs` and `aura-core/src/hypervisor/mod.rs`.
  - `EXE-VM-011`: MicroVM immutable rootfs (`is_read_only: true`, `ro root=/dev/vda`) and ephemeral scratch disk layering in `aura-core/src/hypervisor/firecracker.rs`.
  - 128/128 tests pass in `aura-core`.
  - Independent forensic audit: `CLEAN` verdict from `auditor_m3_2`.
- **Milestone 4 (Iteration 2 Remediation)**:
  - Iteration 1 failed due to defects identified by challengers and auditor (`auditor_m4_1`).
  - Remediation exploration completed by `explorer_m4_fix_1`, `explorer_m4_fix_2`, and `explorer_m4_fix_3`.
  - Artifacts ready to apply:
    - `aura-sdk/aura_sdk/tools/bash.py` <- `/home/jeff/aura/.agents/teamwork/explorer_m4_fix_1/proposed_bash.py`
    - `aura-sdk/aura_sdk/tools/browser.py` <- `/home/jeff/aura/.agents/teamwork/explorer_m4_fix_2/proposed_browser.py`
    - `aura-sdk/tests/test_browser_tool_stress.py` patch <- `/home/jeff/aura/.agents/teamwork/explorer_m4_fix_2/test_stress_remediation.patch`
    - `aura-core/src/hypervisor/firecracker.rs` <- `/home/jeff/aura/.agents/teamwork/explorer_m4_fix_1/proposed_firecracker.rs` (or `/home/jeff/aura/.agents/teamwork/explorer_m4_fix_1/remediation.patch`)
  - Target: All 84/84 pytest tests pass in `aura-sdk/tests`, all `cargo test -p aura-core` pass.
  - Verification: Complete Gate 4 Iteration 2 multi-agent review (Challenger & Forensic Auditor).
- **Milestone 5**: **FINAL MULTI-AGENT VERIFICATION GATE & DELIVERY**
  - Full workspace test suite verification.
  - Final multi-agent audit and victory report.

## Constraints & Rules
- Pure orchestrator: NEVER write production code directly. Dispatch workers and specialists.
- Zero mock / stub policy per `AGENTS.md`. Code must be production-ready.
- Write only to your working directory (`.agents/teamwork/orchestrator_9/`).
- Maintain `BRIEFING.md`, `progress.md`, and `GATE_STATUS.md`.
- Report victory to Sentinel via `send_message` when all acceptance criteria are met and independently verified.
