# Dispatch for Worker M4 Implementation

You are `worker_m4_1` (role: `teamwork_preview_worker`).
Your assigned working directory is `/home/jeff/aura/.agents/teamwork/worker_m4_1/`.

## Mandatory Reading:
- User Request: `/home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md` (R4, R5)
- Master Baseline: `/home/jeff/aura/M0_M3_Audit_Report.md` (§ TOOL-BASH-002, TOOL-BRW-003, GAP-003)
- Scope: `/home/jeff/aura/.agents/teamwork/orchestrator_7/PROJECT.md`
- Implementation Survey: `/home/jeff/aura/.agents/teamwork/explorer_impl_survey_3/handoff.md` (§ 1.1, 1.2, 1.3, 2.1, 2.2, 2.3, 3.1, 3.2, 3.3, 3.4)
- Repository Constitution: `/home/jeff/aura/AGENTS.md` (Strict zero mock/stub policy)
- Domain Skills:
  - `/home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-tool-domain/SKILL.md`
  - `/home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-exe-vm-domain/SKILL.md`
  - `/home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-architecture/SKILL.md`

## Milestone 4 Tasks:
### 1. TOOL-BASH-002: In-Guest Shell Daemon, Host Session Manager, and Python SDK Session Continuity
- Create `deploy/images/vsock-shell-daemon/`:
  - `Cargo.toml`: dependency on `serde`, `serde_json`, `vsock = "0.5"`.
  - `src/session.rs`: `ShellSession` managing persistent `/bin/bash` process with piped stdin/stdout/stderr, executing commands with sentinel delimiters (`__AURA_CWD__`, `__AURA_EXIT__`), capturing stdout, stderr, cwd, and exit code.
  - `src/server.rs`: AF_VSOCK and TCP listener dispatching JSON requests (`BashSessionRequest`) and returning `BashSessionResponse`.
  - `src/main.rs`: entry point parsing `--listen-vsock` (port 5250) and `--listen-tcp` options.
- Update `aura-core/src/hypervisor/firecracker.rs`:
  - Add `DEFAULT_BASH_VSOCK_PORT = 5250`.
  - Implement `BashCommandOutput`, `BashSessionRequest`, `BashSessionHandle`, and `BashSessionManager`.
  - Support multi-call stateful bash session routing without tearing down the microVM between calls.
  - Add unit tests for bash session request/response serialization and session manager.
- Update `aura-sdk/aura_sdk/tools/bash.py`:
  - Update `BashTool.execute(self, command: str, session_id: str | None = None) -> str`
  - Maintain active session state continuity across sequential calls within a session.
  - Add unit tests in `aura-sdk/tests/test_bash_tool.py` verifying env var export and cwd persistence across sequential calls.

### 2. TOOL-BRW-003: Host-to-Guest VSOCK TCP Forwarder & Pure Async Python CDP Client
- Update `aura-core/src/hypervisor/firecracker.rs`:
  - Add `DEFAULT_CDP_VSOCK_PORT = 9222`.
  - Implement `CdpVsockForwarder` proxying host loopback TCP to Firecracker vsock port 9222 via UDS `CONNECT 9222\n`.
  - Add unit tests for CDP forwarder configuration.
- Update `aura-sdk/aura_sdk/tools/browser.py`:
  - Implement `AsyncCdpClient` using installed `httpx` (HTTP endpoints `/json/version`, `/json/list`, `/json/new`) and `websockets` (WebSocket JSON-RPC `Page.navigate`, `Runtime.evaluate`, `Page.captureScreenshot`).
  - Implement `BrowserTool` methods: `navigate(url: str)`, `click(selector: str)`, `evaluate(expression: str)`, `screenshot() -> bytes`, `close()`.
  - Add comprehensive unit tests in `aura-sdk/tests/test_browser_tool.py` covering navigation, click, DOM evaluation, and screenshot capture.

### 3. GAP-003: Formal Architectural Documentation of WASI 0.3 Dynamic Linking Limitation & Static WIT Mitigation
- Update `aura-core/src/hypervisor/wasm.rs`:
  - Add formal doc comments at the top and on `create_linker_with_capabilities()` acknowledging GAP-003, detailing why WASI 0.3 lacks dynamic component linking without host re-instantiation, explaining the static WIT linking mitigation, and outlining criteria for Component Model 1.0 adoption.
- Update `docs/architecture/adrs/ADR-001-Execution-Host.md`:
  - Add dedicated section for GAP-003 with status, context, architectural decision, and roadmap.
- Update `.agents/plugins/aura-dev-kit/skills/aura-architecture/references/02-execution-sandboxes.md`:
  - Add formal GAP-003 specification section.

### 4. Build & Test Verification:
- Run `cargo check --all-targets`
- Run `cargo test -p aura-core` (all tests must pass with 0 failures)
- Run `pytest aura-sdk/tests` (all tests must pass with 0 failures)
- Ensure zero warnings, zero mocks, zero stubs in production code.

Write complete results and test logs in `/home/jeff/aura/.agents/teamwork/worker_m4_1/handoff.md`.

## Mandatory Integrity Warning:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.


## 2026-10-11T03:59:36Z
Received dispatch message:
Role: teamwork_preview_worker
Tasks:
1. TOOL-BASH-002: Build deploy/images/vsock-shell-daemon/, add BashSessionManager/BashSessionHandle in aura-core/src/hypervisor/firecracker.rs, update aura-sdk/aura_sdk/tools/bash.py with multi-call session continuity across calls. Add tests in aura-sdk/tests/test_bash_tool.py.
2. TOOL-BRW-003: Add CdpVsockForwarder in aura-core/src/hypervisor/firecracker.rs, implement pure async AsyncCdpClient and BrowserTool methods (navigate, click, evaluate, screenshot, close) in aura-sdk/aura_sdk/tools/browser.py. Add tests in aura-sdk/tests/test_browser_tool.py.
3. GAP-003: Document WASI 0.3 dynamic runtime component linking limitation and static WIT linking mitigation in aura-core/src/hypervisor/wasm.rs, docs/architecture/adrs/ADR-001-Execution-Host.md, and 02-execution-sandboxes.md.
4. Verify with cargo check --all-targets, cargo test -p aura-core, pytest aura-sdk/tests.
5. Write complete handoff report in .agents/teamwork/worker_m4_1/handoff.md.
6. Send message to orchestrator upon completion.
