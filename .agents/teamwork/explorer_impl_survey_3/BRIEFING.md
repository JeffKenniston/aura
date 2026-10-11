# BRIEFING — 2026-10-10T04:59:00Z

## Mission
Conduct an in-depth codebase survey for TOOL-BASH-002 (persistent bash session), TOOL-BRW-003 (CDP port 9222 forwarder & browser tool client), and GAP-003 (WASI 0.3 dynamic runtime linking limitation doc & static WIT mitigation).

## 🔒 My Identity
- Archetype: explorer
- Roles: read-only investigation, codebase survey, implementation planning
- Working directory: /home/jeff/aura/.agents/teamwork/explorer_impl_survey_3
- Original parent: c402a87d-8780-44d1-b1d0-cba4238c7a72
- Milestone: Survey & Implementation Plan for TOOL-BASH-002, TOOL-BRW-003, GAP-003

## 🔒 Key Constraints
- Read-only investigation — do NOT implement or modify any files outside working directory
- Produce comprehensive handoff.md with observations, logic chain, caveats, conclusion, verification method, proposed structs/APIs, exact files to edit, test design, step-by-step implementation plan
- Update progress.md as work proceeds
- Send message to caller upon completion

## Current Parent
- Conversation ID: c402a87d-8780-44d1-b1d0-cba4238c7a72
- Updated: 2026-10-10T04:59:00Z

## Investigation State
- **Explored paths**:
  - `aura-core/src/hypervisor/firecracker.rs`, `wasm.rs`, `proxy.rs`, `mod.rs`
  - `tools/bash.json`, `tools/browser.json`
  - `deploy/images/build_browser_image.sh`, `build_computer_image.sh`, `vsock-screenshot-daemon/`
  - `aura-sdk/aura_sdk/tools/bash.py`, `browser.py`, `__init__.py`
  - `aura-sdk/tests/test_transport.py`, `test_bus_client.py`, and virtualenv packages
  - `docs/architecture/adrs/ADR-001-Execution-Host.md`, `references/02-execution-sandboxes.md`
- **Key findings**:
  - TOOL-BASH-002: Currently stateless one-shot execution. Requires in-guest `vsock-shell-daemon` on AF_VSOCK port 5250, active `BashSessionHandle` & `BashSessionManager` in `firecracker.rs`, and session ID routing in `bash.py`.
  - TOOL-BRW-003: Chromium exposes CDP on port 9222 inside guest image. Requires host-to-guest `CdpVsockForwarder` in `firecracker.rs` proxying loopback TCP to vsock UDS port 9222. Python SDK has `httpx` and `websockets` pre-installed, enabling pure async `AsyncCdpClient` and complete `BrowserTool` actions (`navigate`, `click`, `evaluate`, `screenshot`).
  - GAP-003: WASI 0.3 runtime dynamic linking is unstandardized upstream. Mitigation is deploy-time static WIT component linking. Needs documentation and comments in `wasm.rs`, `ADR-001`, and `02-execution-sandboxes.md`.
- **Unexplored areas**: None for survey scope; comprehensive blueprint delivered in `handoff.md`.

## Key Decisions Made
- Architected `vsock-shell-daemon` in `deploy/images/vsock-shell-daemon` following existing `vsock-screenshot-daemon` pattern with dual vsock / TCP support for testability.
- Designed `BashSessionHandle` and `BashSessionManager` in `firecracker.rs` for stateful session persistence.
- Designed `CdpVsockForwarder` in `firecracker.rs` and pure async `AsyncCdpClient` in `browser.py` using pre-installed `httpx` and `websockets`.
- Formulated static WIT linking documentation for `GAP-003` in `wasm.rs` and ADR-001.

## Artifact Index
- /home/jeff/aura/.agents/teamwork/explorer_impl_survey_3/DISPATCH.md — Dispatch instructions
- /home/jeff/aura/.agents/teamwork/explorer_impl_survey_3/BRIEFING.md — Context and situational awareness
- /home/jeff/aura/.agents/teamwork/explorer_impl_survey_3/progress.md — Liveness heartbeat and task progress
- /home/jeff/aura/.agents/teamwork/explorer_impl_survey_3/handoff.md — Detailed survey report & implementation blueprint
