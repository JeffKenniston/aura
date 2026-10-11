# Dispatch: Explorer M4.2 (TOOL-BRW-003 Chromium CDP VSOCK Proxy & BrowserTool)

**Agent ID**: `explorer_m4_2`  
**Working Directory**: `/home/jeff/aura/.agents/teamwork/explorer_m4_2/`  
**Target Milestone**: Milestone 4: Persistent Stateful Tools & GAP-003  
**Focus**: `TOOL-BRW-003` (Host-to-guest vsock TCP proxy connecting Chromium CDP port 9222 and async Python CDP client)

## Objectives
1. Read `/home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md` (MANDATORY).
2. Read `/home/jeff/aura/AGENTS.md`.
3. Investigate the current state of:
   - `aura-sdk/aura_sdk/tools/browser.py`
   - `aura-sdk/tests/test_browser_tool.py` (or existing tests in `aura-sdk/tests`)
   - `aura-core/src/hypervisor/firecracker.rs` (vsock port 9222 forwarding)
4. Determine how host-to-guest vsock TCP forwarding for Chromium CDP port 9222 and the async Python CDP client (`BrowserTool`) should be implemented authentically without mocks, so basic browser automation actions work and pytest tests pass.
5. Provide precise implementation strategy, file citations, and verification method.
6. Write full findings to `/home/jeff/aura/.agents/teamwork/explorer_m4_2/handoff.md` and notify parent.

## 2026-10-11T04:23:44Z
You are Explorer M4.2 investigating TOOL-BRW-003 (Chromium CDP vsock proxy and async Python BrowserTool) for Milestone 4 in Aura.
Working Directory: `/home/jeff/aura/.agents/teamwork/explorer_m4_2/`
Mandate: Investigate `aura-sdk/aura_sdk/tools/browser.py`, `aura-sdk/tests/test_browser_tool.py`, `aura-core/src/hypervisor/firecracker.rs`, vsock port 9222 forwarding, and async Python CDP client (`BrowserTool`).
