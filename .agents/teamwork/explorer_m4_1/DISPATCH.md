# Dispatch: Explorer M4.1 (TOOL-BASH-002 Persistent VSOCK Shell)

**Agent ID**: `explorer_m4_1`  
**Working Directory**: `/home/jeff/aura/.agents/teamwork/explorer_m4_1/`  
**Target Milestone**: Milestone 4: Persistent Stateful Tools & GAP-003  
**Focus**: `TOOL-BASH-002` (Persistent AF_VSOCK shell daemon for bash session state continuity)

## Objectives
1. Read `/home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md` (MANDATORY).
2. Read `/home/jeff/aura/AGENTS.md`.
3. Investigate the current state of:
   - `aura-sdk/aura_sdk/tools/bash.py`
   - `aura-sdk/tests/test_bash_tool.py` (or existing tests in `aura-sdk/tests`)
   - `deploy/images/` or any vsock daemon code/scripts
   - `aura-core/src/hypervisor/firecracker.rs` (vsock configuration and port mappings)
4. Determine how bash session state continuity (env vars and cwd persisting across sequential invocations with the same session ID) should be implemented cleanly, authentically, and without mocks.
5. Provide precise implementation strategy, file citations, and verification method.
6. Write full findings to `/home/jeff/aura/.agents/teamwork/explorer_m4_1/handoff.md` and notify parent.


## 2026-10-11T04:23:44Z

You are Explorer M4.1 investigating TOOL-BASH-002 (persistent vsock shell daemon and session continuity) for Milestone 4 in Aura.

Working Directory: `/home/jeff/aura/.agents/teamwork/explorer_m4_1/`
Always create and maintain your `progress.md` and `BRIEFING.md` in your working directory.

MANDATORY:
1. Read `/home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md` before starting work.
2. Read `/home/jeff/aura/.agents/teamwork/explorer_m4_1/DISPATCH.md`.
3. Read `/home/jeff/aura/AGENTS.md`.

Investigate:
- `aura-sdk/aura_sdk/tools/bash.py`
- `aura-sdk/tests/test_bash_tool.py` or existing bash tests in `aura-sdk/tests`
- `aura-core/src/hypervisor/firecracker.rs` (vsock configuration and port mappings)
- Session state continuity (env vars and cwd persisting across sequential calls with the same session ID).

Provide concrete, production-ready implementation recommendations (no mocks/stubs) and write your findings to `/home/jeff/aura/.agents/teamwork/explorer_m4_1/handoff.md`.
Send message to parent when done.
