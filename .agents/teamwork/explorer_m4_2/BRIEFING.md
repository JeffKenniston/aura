# BRIEFING — 2026-10-11T04:31:30Z

## Mission
Investigate TOOL-BRW-003: Chromium CDP vsock proxy and async Python BrowserTool for Milestone 4 in Aura.

## 🔒 My Identity
- Archetype: explorer
- Roles: investigator, synthesizer
- Working directory: /home/jeff/aura/.agents/teamwork/explorer_m4_2/
- Original parent: ad091f9f-df8d-4332-bf82-a7111f5328d2
- Milestone: Milestone 4 (Persistent Stateful Tools & GAP-003)

## 🔒 Key Constraints
- Read-only investigation — do NOT implement
- Verification Gates: Code must be production-ready. ABSOLUTELY NO mocks, stubs, or TODOs.
- Circuit Breaker: If a task fails or errors continuously, do not blindly retry permutations. Halt and ask the user for clarification.
- Host-to-guest vsock TCP forwarding connecting Chromium CDP port 9222 and async Python CDP client (BrowserTool).

## Current Parent
- Conversation ID: ad091f9f-df8d-4332-bf82-a7111f5328d2
- Updated: not yet

## Investigation State
- **Explored paths**:
  - `aura-core/src/hypervisor/firecracker.rs` (vsock virtio socket paths, screenshot listener, VmConfig::for_browser)
  - `aura-sdk/aura_sdk/tools/browser.py` (current 10-line stub)
  - `aura-sdk/pyproject.toml` and `.venv` (verified websockets 16.1.1 and httpx 0.28.1 installed)
  - `deploy/images/build_browser_image.sh` (Chromium CLI launch flags, remote-debugging-port 9222)
  - `deploy/images/vsock-screenshot-daemon/` (vsock IPC reference)
- **Key findings**:
  - Firecracker vsock host-to-guest communication requires host to connect to UDS `vsock_uds_path` and send `CONNECT <port>\n`.
  - A dedicated host-to-guest TCP forwarder (`CdpVsockForwarder`) in `firecracker.rs` bridges TCP connections on `127.0.0.1:<port>` to Firecracker's UDS via `CONNECT 9222\n` and `tokio::io::copy_bidirectional`.
  - In `aura-sdk`, `AsyncCdpClient` and `BrowserTool` can implement `navigate()`, `click()`, `evaluate()`, and `screenshot()` using `httpx` (for target discovery) and `websockets` (for JSON-RPC commands).
  - In `aura-sdk/tests/test_browser_tool.py`, tests can run against an in-process genuine combined HTTP/WS server using `websockets.asyncio.server.serve`, eliminating mocks in production code while thoroughly validating CDP protocols.
- **Unexplored areas**: None remaining for TOOL-BRW-003.

## Key Decisions Made
- Use single-byte read loop for Firecracker `CONNECT` handshake in `firecracker.rs` to avoid `BufReader` over-buffering payload data.
- Use `websockets` + `httpx` natively in `aura-sdk` without external browser automation binaries (playwright/chromiumoxide) since both are already present in the virtualenv.

## Artifact Index
- /home/jeff/aura/.agents/teamwork/explorer_m4_2/progress.md — Liveness & status tracking
- /home/jeff/aura/.agents/teamwork/explorer_m4_2/DISPATCH.md — Incoming directives
- /home/jeff/aura/.agents/teamwork/explorer_m4_2/handoff.md — Final investigation report
