# BRIEFING — 2026-10-11T04:36:00Z

## Mission
Investigate TOOL-BASH-002: Persistent AF_VSOCK shell daemon and session continuity (env vars and cwd persisting across sequential calls with the same session ID) for Milestone 4 in Aura.

## 🔒 My Identity
- Archetype: explorer
- Roles: investigation, analysis, synthesis
- Working directory: /home/jeff/aura/.agents/teamwork/explorer_m4_1
- Original parent: ad091f9f-df8d-4332-bf82-a7111f5328d2
- Milestone: Milestone 4 (Persistent Stateful Tools & GAP-003)

## 🔒 Key Constraints
- Read-only investigation — do NOT implement
- Production-ready recommendations: ABSOLUTELY NO mocks, stubs, or TODOs (AGENTS.md)
- Follow Handoff Protocol (5 components: Observation, Logic Chain, Caveats, Conclusion, Verification Method) in `handoff.md`
- Maintain `progress.md` heartbeat

## Current Parent
- Conversation ID: ad091f9f-df8d-4332-bf82-a7111f5328d2
- Updated: not yet

## Investigation State
- **Explored paths**:
  - `/home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md` (R4, R5 requirements & acceptance criteria)
  - `/home/jeff/aura/.agents/teamwork/explorer_m4_1/DISPATCH.md`
  - `/home/jeff/aura/AGENTS.md` (Zero mock/stub rule)
  - `/home/jeff/aura/aura-sdk/aura_sdk/tools/bash.py` (lines 1-10: stateless stub without session ID)
  - `/home/jeff/aura/aura-sdk/tests/` (verified 48/48 tests pass; confirmed `test_bash_tool.py` missing)
  - `/home/jeff/aura/aura-core/src/hypervisor/firecracker.rs` (vsock UDS configuration, guest CID 3, host CID 2, port mapping conventions)
  - `/home/jeff/aura/deploy/images/` (`vsock-screenshot-daemon` verified & tested, `build_computer_image.sh`, `build_browser_image.sh`)
  - `/home/jeff/aura/tools/bash.json` (documented legacy ephemeral destruction)
- **Key findings**:
  - `TOOL-BASH-002` requires 3 coordinated layers:
    1. In-guest `vsock-shell-daemon` (`deploy/images/vsock-shell-daemon/`) maintaining persistent `/bin/bash` processes with sentinel framing and JSON line-delimited IPC over AF_VSOCK port 5250 (and TCP fallback).
    2. Host hypervisor session management in `aura-core/src/hypervisor/firecracker.rs`: `DEFAULT_BASH_VSOCK_PORT = 5250`, `BashSessionRequest`, `BashCommandOutput`, `BashSessionHandle`, and `BashSessionManager` with multi-call routing and 125s inactivity eviction.
    3. Python SDK `BashTool` in `aura-sdk/aura_sdk/tools/bash.py`: accepts `session_id`, persists `env` and `cwd` state across sequential calls with same session ID, and comprehensive tests in `aura-sdk/tests/test_bash_tool.py`.
- **Unexplored areas**: None for TOOL-BASH-002 scope. Ready to author `handoff.md`.

## Key Decisions Made
- Architecture blueprint codified: pure Rust in-guest daemon using standard pipes and UUID-sentinel execution detection, line-buffered JSON framing over vsock port 5250, and authentic subprocess session tracking in Python SDK and Rust hypervisor manager.

## Artifact Index
- `/home/jeff/aura/.agents/teamwork/explorer_m4_1/BRIEFING.md` — persistent memory index
- `/home/jeff/aura/.agents/teamwork/explorer_m4_1/progress.md` — heartbeat and progress tracker
- `/home/jeff/aura/.agents/teamwork/explorer_m4_1/handoff.md` — final 5-component handoff report
