# Worker Dispatch: Milestone 4 (TOOL-BASH-002, TOOL-BRW-003, GAP-003)

**Worker ID**: `worker_m4_3`  
**Working Directory**: `/home/jeff/aura/.agents/teamwork/worker_m4_3/`  
**Target Milestone**: Milestone 4: Persistent Stateful Tools & GAP-003  
**Integrity Mode**: Demo (per `ORIGINAL_REQUEST.md`)

## Mandatory Directives
MANDATORY INTEGRITY WARNING:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

Repository Constitution:
Read and adhere strictly to `/home/jeff/aura/AGENTS.md`. Code must be production-ready. ABSOLUTELY NO mocks, stubs, or TODOs.

## Inputs to Read
1. `/home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md` (MANDATORY — read first)
2. `/home/jeff/aura/AGENTS.md`
3. Explorer blueprints:
   - `/home/jeff/aura/.agents/teamwork/explorer_m4_1/handoff.md` (TOOL-BASH-002)
   - `/home/jeff/aura/.agents/teamwork/explorer_m4_2/handoff.md` (TOOL-BRW-003)
   - `/home/jeff/aura/.agents/teamwork/spec_miner_m4_1/handoff.md` (GAP-003)

## File Ownership
You have exclusive write access to:
- `deploy/images/vsock-shell-daemon/*`
- `aura-core/src/hypervisor/firecracker.rs`
- `aura-core/src/hypervisor/mod.rs`
- `aura-core/src/hypervisor/wasm.rs`
- `docs/architecture/adrs/ADR-001-Execution-Host.md`
- `.agents/plugins/aura-dev-kit/skills/aura-architecture/references/02-execution-sandboxes.md`
- `aura-sdk/aura_sdk/tools/bash.py`
- `aura-sdk/aura_sdk/tools/browser.py`
- `aura-sdk/tests/test_bash_tool.py`
- `aura-sdk/tests/test_browser_tool.py`

## Implementation Tasks

### 1. TOOL-BASH-002: In-Guest Shell Daemon (`deploy/images/vsock-shell-daemon/`)
- Implement `Cargo.toml`, `src/main.rs`, `src/session.rs`, and `src/server.rs`.
- Support single-line JSON IPC over AF_VSOCK port 5250 (with `--tcp` fallback mode for host testing).
- Track persistent child `/bin/bash` sessions, isolating stdout, stderr, exit code, and updated `cwd` using sentinel output markers.
- Verify with `cargo test --manifest-path deploy/images/vsock-shell-daemon/Cargo.toml`.

### 2. TOOL-BASH-002: Microkernel Session Management (`aura-core/src/hypervisor/firecracker.rs`)
- Add `DEFAULT_BASH_VSOCK_PORT = 5250`.
- Add `BashSessionRequest`, `BashCommandOutput`, `BashSessionHandle`, and `BashSessionManager`.
- Maintain active microVMs per `session_id`, forwarding commands through Firecracker vsock UDS (`CONNECT 5250\n`), with an inactivity timeout of 125 seconds.
- Provide unit tests in `firecracker.rs` verifying session handle management and command execution.

### 3. TOOL-BRW-003: Host-to-Guest VSOCK TCP Forwarder (`aura-core/src/hypervisor/firecracker.rs`)
- Add `DEFAULT_CDP_VSOCK_PORT = 9222`.
- Implement `CdpVsockForwarder` binding a host loopback TCP listener and bridging streams to Firecracker vsock UDS (`CONNECT 9222\n`).
- Implement `start_cdp_forwarder` and `stop_cdp_forwarder` in `FirecrackerVm`.
- Add unit tests verifying forwarder lifecycle.

### 4. TOOL-BASH-002: Python SDK Stateful `BashTool` (`aura-sdk/aura_sdk/tools/bash.py`)
- Replace the stateless stub with a stateful implementation accepting `session_id`.
- Persist working directory and environment variables across sequential calls sharing the same session ID.
- Provide `execute`, `new_session`, `get_session_id`, and `reset` methods.
- Add comprehensive pytest tests in `aura-sdk/tests/test_bash_tool.py`.

### 5. TOOL-BRW-003: Python SDK Async `BrowserTool` (`aura-sdk/aura_sdk/tools/browser.py`)
- Replace the 10-line stub with an async CDP client using `httpx` and `websockets`.
- Implement `navigate`, `click`, `evaluate`, `screenshot`, and `close`.
- Add comprehensive pytest tests in `aura-sdk/tests/test_browser_tool.py`.

### 6. GAP-003: Architectural Documentation
- Add module and function doc comments in `aura-core/src/hypervisor/wasm.rs` explaining the WASI 0.3 dynamic runtime linking limitation and the static WIT linking mitigation.
- Append Decision 5 to `docs/architecture/adrs/ADR-001-Execution-Host.md`.
- Update Section 1 of `.agents/plugins/aura-dev-kit/skills/aura-architecture/references/02-execution-sandboxes.md`.

## Verification Commands
Before submitting your handoff, you must run and verify:
1. `cargo check --all-targets` (Must pass with 0 warnings, 0 errors).
2. `cargo test -p aura-core` (All tests must pass, 0 failed).
3. `cargo test --manifest-path deploy/images/vsock-shell-daemon/Cargo.toml` (All tests must pass).
4. `source aura-sdk/.venv/bin/activate && pytest aura-sdk/tests` (All tests must pass, 0 failed).


## 2026-10-11T04:32:58Z
You are Worker M4.3 implementing Milestone 4: Persistent Stateful Tools & GAP-003 for the Aura project.
Working directory: /home/jeff/aura/.agents/teamwork/worker_m4_3/
All tasks assigned per dispatch.
