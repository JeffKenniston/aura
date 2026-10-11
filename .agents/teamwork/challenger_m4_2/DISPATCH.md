# Challenger Dispatch: Milestone 4 Challenger 2 (Browser CDP Proxy & Automation Empirical Stress)

**Agent ID**: `challenger_m4_2`  
**Working Directory**: `/home/jeff/aura/.agents/teamwork/challenger_m4_2/`  
**Target Milestone**: Milestone 4: Persistent Stateful Tools & GAP-003  
**Focus**: Empirical Verification of Chromium CDP Vsock Proxy & Async BrowserTool Automation

## Objectives
1. Read `/home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md` (MANDATORY).
2. Read `/home/jeff/aura/AGENTS.md`.
3. Read `/home/jeff/aura/.agents/teamwork/worker_m4_3/handoff.md`.
4. Empirically verify `TOOL-BRW-003`:
   - Verify `CdpVsockForwarder` in Rust:
     - Handshake parsing (`OK <port>\n`)
     - Bidirectional streaming transfer
     - Handshake error handling
   - Verify `BrowserTool` in Python SDK:
     - Navigation (`navigate`)
     - Element click and selector error handling (`click`)
     - JavaScript evaluation (`evaluate`)
     - Screenshot capturing with valid PNG magic bytes (`screenshot`)
     - Clean disconnection (`close`)
5. Run tests:
   - `cargo test -p aura-core --lib hypervisor::firecracker::tests::test_cdp_vsock_forwarder`
   - `source aura-sdk/.venv/bin/activate && pytest aura-sdk/tests/test_browser_tool.py`
6. Write your structured challenger report and verdict (`APPROVE` or `REJECT`) to `/home/jeff/aura/.agents/teamwork/challenger_m4_2/handoff.md` and notify parent.

## 2026-10-11T04:59:30Z
You are Challenger 2 for Milestone 4 in the Aura project, focusing on TOOL-BRW-003.

Working Directory: `/home/jeff/aura/.agents/teamwork/challenger_m4_2/`
Always create and maintain your `progress.md` and `BRIEFING.md` in your working directory.

MANDATORY:
1. Read `/home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md` before starting work.
2. Read `/home/jeff/aura/.agents/teamwork/challenger_m4_2/DISPATCH.md`.
3. Read `/home/jeff/aura/AGENTS.md`.
4. Read `/home/jeff/aura/.agents/teamwork/worker_m4_3/handoff.md`.

Empirically stress-test browser CDP proxy and automation:
- Verify CdpVsockForwarder handshake and bidirectional streaming
- Verify BrowserTool actions: navigate, click, evaluate, screenshot, close
- Verify error handling: invalid selector, network timeout, WebSocket disconnect
- Run `cargo test -p aura-core --lib hypervisor::firecracker::tests::test_cdp_vsock_forwarder`
- Run `source aura-sdk/.venv/bin/activate && pytest aura-sdk/tests/test_browser_tool.py`

Write report to `/home/jeff/aura/.agents/teamwork/challenger_m4_2/handoff.md`.
Send message with verdict (APPROVE or REJECT) to parent.
