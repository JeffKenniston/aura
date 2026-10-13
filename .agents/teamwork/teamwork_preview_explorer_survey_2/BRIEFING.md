# BRIEFING — 2026-10-11T14:56:00Z

## Mission
Survey requirement R2 (Sandboxed GUI Automation & Visual Inspection with Redaction) for Phase M5 (Hardening) in Aura.

## 🔒 My Identity
- Archetype: explorer
- Roles: investigation, synthesis
- Working directory: /home/jeff/aura/.agents/teamwork/teamwork_preview_explorer_survey_2
- Original parent: dae2eddf-f903-4d1f-b5f4-42a3e7019103
- Milestone: M5 (Hardening) - Requirement R2 Survey

## 🔒 Key Constraints
- Read-only investigation — do NOT implement
- Base code on production standards: ABSOLUTELY NO mocks, stubs, or TODOs
- Follow file workspace convention (.agents/teamwork/<folder>/ holds only metadata)
- Output survey_report.md and handoff.md in working directory
- Communicate completion and findings via send_message to caller

## Current Parent
- Conversation ID: dae2eddf-f903-4d1f-b5f4-42a3e7019103
- Updated: 2026-10-11T14:56:00Z

## Investigation State
- **Explored paths**:
  - `deploy/images/vsock-screenshot-daemon/` (`Cargo.toml`, `Cargo.lock`, `src/main.rs`, `src/capture.rs`, `src/server.rs`)
  - `deploy/images/build_computer_image.sh`, `deploy/images/README.md`, `tools/computer.json`
  - `aura-core/src/hypervisor/firecracker.rs` (`extract_png_bytes`, `process_and_save_screenshot`, `start_screenshot_listener`, `CdpVsockForwarder`)
  - `aura-core/src/runtime/registry.rs`, `aura-core/src/bus/mod.rs`, `aura-core/src/transport/mcp.rs`
  - `aura-sdk/aura_sdk/tools/computer.py`, `tools/browser.py`, `tools/bash.py`
  - `aura-sdk/tests/` (84 passing tests)
- **Key findings**:
  1. `vsock-screenshot-daemon` currently only supports capturing screenshots on connect. It lacks JSON-RPC input handling completely.
  2. Input actions (`click`, `move`, `type`, `key`, `drag`) need a new input module and command parsing loop without dropping events.
  3. `capture.rs` has an X11 `BadMatch` bug caused by passing `!0` as `plane_mask` on 24-bit root windows (violates X11 spec).
  4. Host-side redaction masks do not exist yet in either `firecracker.rs` or `aura-sdk`. Pure Python `zlib` + `struct` PNG redaction demonstrated zero-dependency success.
  5. `aura-sdk/aura_sdk/tools/computer.py` is a 10-line stub with only `click`. Needs full async mouse, typing, key, drag, screenshot, and redaction mask support.
  6. Port discrepancy: `firecracker.rs` defines `DEFAULT_SCREENSHOT_VSOCK_PORT = 5252`, while `build_computer_image.sh` and `vsock-screenshot-daemon` use `5254`.
- **Unexplored areas**: None for R2 scope.

## Key Decisions Made
- Fully documented file boundaries, interface schemas, existing vs missing gap analysis, and implementation roadmap.

## Artifact Index
- /home/jeff/aura/.agents/teamwork/teamwork_preview_explorer_survey_2/DISPATCH.md — Incoming dispatches
- /home/jeff/aura/.agents/teamwork/teamwork_preview_explorer_survey_2/progress.md — Heartbeat & execution log
- /home/jeff/aura/.agents/teamwork/teamwork_preview_explorer_survey_2/BRIEFING.md — Working memory
- /home/jeff/aura/.agents/teamwork/teamwork_preview_explorer_survey_2/survey_report.md — Comprehensive survey report
- /home/jeff/aura/.agents/teamwork/teamwork_preview_explorer_survey_2/handoff.md — 5-component handoff report
