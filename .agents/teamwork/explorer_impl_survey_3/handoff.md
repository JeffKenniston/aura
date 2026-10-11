# Implementation Survey & Architecture Blueprint: TOOL-BASH-002, TOOL-BRW-003, GAP-003

## Executive Summary
This survey provides an exhaustive forensic investigation and engineering blueprint for implementing the remaining stateful sandbox and architectural requirements of Phase M2 & M3:
1. **TOOL-BASH-002**: Multi-call bash session persistence across sequential invocations via an in-guest `vsock-shell-daemon` listening on `AF_VSOCK` port 5250, active session management in `aura-core/src/hypervisor/firecracker.rs` (`BashSessionHandle`, `BashSessionManager`), and session-routed execution in `aura-sdk/aura_sdk/tools/bash.py`.
2. **TOOL-BRW-003**: Headless Chromium Chrome DevTools Protocol (CDP) port 9222 host-to-guest vsock TCP forwarding in `firecracker.rs` (`CdpVsockForwarder`), pure async Python CDP driver in `aura-sdk/aura_sdk/tools/browser.py` (`AsyncCdpClient`, `BrowserTool`), and comprehensive async test coverage in `aura-sdk/tests/test_browser_tool.py`.
3. **GAP-003**: Formal architectural acknowledgment and documentation of the WASI 0.3 dynamic runtime linking limitation in `aura-core/src/hypervisor/wasm.rs`, `ADR-001-Execution-Host.md`, and `02-execution-sandboxes.md`, codifying the static WIT component linking mitigation and roadmap to WASI Component Model 1.0.

---

## 1. Observations

### 1.1 TOOL-BASH-002: Current State & Verbatim Codebase Evidence
- **Requirement (`requirements.md:175`)**:
  > `TOOL-BASH-002`: Preserve session state across sequential bash calls within an active task scope while enforcing execution timeouts (125s) and memory caps (128MiB).
- **Manifest Definition (`tools/bash.json:5-6`)**:
  ```json
  "description": "Ephemeral shell execution environment destroyed immediately post-execution",
  "timeoutMs": 125000,
  ```
  The tool manifest explicitly documents the legacy one-shot ephemeral teardown model.
- **Python SDK Implementation (`aura-sdk/aura_sdk/tools/bash.py:4-10`)**:
  ```python
  class BashTool:
      def __init__(self, transport: ZenohClient):
          self.transport = transport

      async def execute(self, command: str) -> str:
          return await self.transport.dispatch_tool("bash", {"command": command})
  ```
  `BashTool.execute` is stateless and accepts only `command: str`. It lacks any `session_id` parameter or session lifecycle tracking.
- **MicroVM Hypervisor (`aura-core/src/hypervisor/firecracker.rs:538-542`)**:
  ```rust
  impl Drop for FirecrackerVm {
      fn drop(&mut self) {
          let _ = self.teardown();
      }
  }
  ```
  `FirecrackerVm` provides only immediate teardown. There is no active session handle, persistent vsock shell connection, or multi-command session cache across invocations.
- **Existing Daemon Pattern (`deploy/images/vsock-screenshot-daemon/`)**:
  `vsock-screenshot-daemon` demonstrates how guest daemons communicate over `AF_VSOCK` with Firecracker:
  - Crate dependency `vsock = "0.5"` (`Cargo.toml:14`).
  - Supports `--listen` on AF_VSOCK and `--tcp-listen` for fallback host testing (`main.rs:46-54`).

### 1.2 TOOL-BRW-003: Current State & Verbatim Codebase Evidence
- **Requirement (`requirements.md:184`)**:
  > `TOOL-BRW-003`: Headless Chromium exposes Chrome DevTools Protocol (CDP) on port 9222 for remote navigation, DOM evaluation, and page scraping, controlled by `BrowserTool`.
- **Guest Rootfs Builder (`deploy/images/build_browser_image.sh:196-197`)**:
  ```bash
  exec chromium \
    --headless=new \
    --no-sandbox \
    --disable-gpu \
    --disable-dev-shm-usage \
    --disable-software-rasterizer \
    --remote-debugging-port=9222 \
    --remote-debugging-address=0.0.0.0 \
    ...
  ```
  Headless Chromium inside `browser.ext4` is configured to listen on port 9222 TCP.
- **Hypervisor Forwarding Gap (`aura-core/src/hypervisor/firecracker.rs:375-389`)**:
  `FirecrackerVm` exposes only guest-to-host screenshot streaming (`start_screenshot_listener` on vsock port 5252). There is no host-to-guest TCP-to-vsock forwarding bridge for port 9222.
- **Python SDK Implementation (`aura-sdk/aura_sdk/tools/browser.py:4-10`)**:
  ```python
  class BrowserTool:
      def __init__(self, transport: ZenohClient):
          self.transport = transport

      async def navigate(self, url: str) -> str:
          return await self.transport.dispatch_tool("browser", {"url": url})
  ```
  `BrowserTool` is a 10-line stub with only a single method (`navigate`) that delegates to Zenoh. It contains no CDP client, no WebSocket transport, and no actions for `click`, `evaluate`, `screenshot`, or `close`.
- **Installed Dependencies (`aura-sdk/.venv`)**:
  `pip list` verifies that `httpx 0.28.1` and `websockets 16.1.1` are already installed in `/home/jeff/aura/aura-sdk/.venv`. A pure async CDP client can be built with zero additional dependencies.

### 1.3 GAP-003: Current State & Verbatim Codebase Evidence
- **Requirement (`requirements.md:238`)**:
  > `GAP-003`: Standard WASI 0.3 lacks standardized dynamic runtime hot-linking of newly generated WASM components without pre-composition or host restart.
- **Microkernel WASM Engine (`aura-core/src/hypervisor/wasm.rs:65-75, 92-117`)**:
  `wasm.rs` utilizes `wasmtime::component::Linker` with static linking (`wasmtime_wasi::add_to_linker_async(&mut linker)`). There are currently no doc comments acknowledging `GAP-003`, explaining the static WIT linking mitigation, or detailing the migration path to Component Model 1.0.
- **Architecture References (`references/02-execution-sandboxes.md:19`)**:
  ```markdown
  - **Dynamic Linking Restriction**: Static component composition post-deployment must be utilized until Component Model 1.0 formally standardizes dynamic host linking.
  ```
  The restriction is noted in a single bullet, but lacks explicit tracking under `GAP-003` and does not describe the Bytecode Alliance specification boundary.

---

## 2. Logic Chain

1. **Stateful Bash Execution Requires In-Guest Process Continuity**:
   - Because `FirecrackerVm` boots a Linux kernel, running a new shell command via a fresh boot or fresh process loses all process memory, export variables (`export FOO=bar`), and current working directory (`cd /tmp`).
   - Maintaining state across consecutive calls requires a resident daemon inside the guest (`bash-session-daemon` / `vsock-shell-daemon`) that manages a persistent child `/bin/bash` process per `session_id`.
   - The host hypervisor (`firecracker.rs`) must maintain an active connection handle (`BashSessionHandle`) and a session registry (`BashSessionManager`) that routes incoming commands with the same `session_id` to the active microVM rather than tearing it down after each call.
   - The Python SDK (`bash.py`) must propagate `session_id` in its API and maintain stateful session continuity both when connected to the microkernel and in standalone/mock test environments.

2. **CDP Requires Bidirectional Host-to-Guest Multiplexing**:
   - Chromium in the guest listens on TCP port 9222.
   - Firecracker provides host-guest IPC strictly through `AF_VSOCK` via a Unix Domain Socket on the host (`vsock_uds_path`).
   - In Firecracker's virtio-vsock protocol, when a host process connects to `vsock_uds_path` and issues `CONNECT 9222\n`, Firecracker proxies that connection to port 9222 inside the guest.
   - A host-side TCP proxy (`CdpVsockForwarder`) that listens on `127.0.0.1:9222` (or an ephemeral loopback port) and bridges accepted TCP connections to Firecracker's vsock UDS allows standard HTTP and WebSocket CDP clients on the host to interact with Chromium seamlessly.
   - Since `httpx` and `websockets` are already installed in `aura-sdk`, implementing an `AsyncCdpClient` inside `browser.py` provides complete CDP capabilities (`Page.navigate`, `Runtime.evaluate`, `Page.captureScreenshot`, and click evaluation) without heavy external packages like playwright.

3. **GAP-003 Is an Upstream Specification Boundary Requiring Clear Documentation**:
   - Dynamic linking of arbitrary WebAssembly components in memory at runtime is currently unstandardized in the Bytecode Alliance WASI 0.3 specification.
   - Aura's established architectural mitigation is deploy-time static WIT linking (via `wit-bindgen` and `wasmtime::component::Linker`).
   - Satisfying GAP-003 requires formal documentation and code comments in `wasm.rs`, `ADR-001`, and `02-execution-sandboxes.md` stating the limitation, justifying the static WIT mitigation, and defining the criteria for adopting Component Model 1.0 dynamic linking.

---

## 3. Concrete Proposed Structs, APIs, and Schemas

### 3.1 `deploy/images/vsock-shell-daemon/`
Standalone Rust package running inside `bash.ext4` (or host TCP test mode).

#### Protocol Framing (JSON over AF_VSOCK / TCP)
- **Request (`BashSessionRequest`)**:
  ```json
  {
    "session_id": "sess-uuid-1234",
    "command": "export AURA_TEST=42 && cd /tmp",
    "action": "execute"
  }
  ```
  `action` can be `"execute"`, `"close"`, or `"ping"`.
- **Response (`BashSessionResponse`)**:
  ```json
  {
    "session_id": "sess-uuid-1234",
    "stdout": "...",
    "stderr": "...",
    "exit_code": 0,
    "cwd": "/tmp"
  }
  ```

#### Core Components
- `Cargo.toml`:
  ```toml
  [package]
  name = "vsock-shell-daemon"
  version = "0.1.0"
  edition = "2021"
  authors = ["Jeff Kenniston"]
  license = "Apache-2.0"
  description = "Persistent in-guest shell daemon piping bash session state over AF_VSOCK (TOOL-BASH-002)"

  [dependencies]
  serde = { version = "1.0", features = ["derive"] }
  serde_json = "1.0"
  vsock = "0.5"
  ```
- `src/session.rs`: `ShellSession`
  - Spawns persistent `/bin/bash` with piped stdin/stdout/stderr.
  - On `execute(cmd)`: sends `cmd` followed by execution sentinels:
    ```bash
    echo "\n__AURA_CWD__:"$(pwd)
    echo "__AURA_EXIT__:$?"
    ```
  - Parses output, extracts `stdout`, `stderr`, current `cwd`, and `exit_code`.
- `src/server.rs`:
  - `run_vsock_listener(port: u32)` for in-guest execution.
  - `run_tcp_listener(port: u16)` for host unit testing.

---

### 3.2 `aura-core/src/hypervisor/firecracker.rs`

#### New Constants & Structs
```rust
/// Default Firecracker vsock port for the persistent bash session daemon (TOOL-BASH-002).
pub const DEFAULT_BASH_VSOCK_PORT: u32 = 5250;

/// Default Firecracker vsock port for the Chrome DevTools Protocol (CDP) forwarder (TOOL-BRW-003).
pub const DEFAULT_CDP_VSOCK_PORT: u32 = 9222;

/// Output of a bash session command execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BashCommandOutput {
    pub session_id: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub cwd: Option<String>,
}

/// Request sent to the in-guest bash daemon over vsock.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BashSessionRequest {
    pub session_id: String,
    pub command: String,
    pub action: String,
}

/// Active vsock session handle maintaining guest microVM state (TOOL-BASH-002).
pub struct BashSessionHandle {
    pub session_id: String,
    pub vm: FirecrackerVm,
    pub vsock_port: u32,
    pub vsock_stream: Option<tokio::net::UnixStream>,
    pub last_accessed: std::time::Instant,
}

impl BashSessionHandle {
    /// Creates and boots a microVM for the session, connecting to the in-guest vsock daemon.
    pub async fn new(
        session_id: &str,
        vm_config: &VmConfig,
        vsock_port: u32,
    ) -> Result<Self, Box<dyn Error + Send + Sync>>;

    /// Executes a shell command within the active persistent session.
    pub async fn execute(
        &mut self,
        command: &str,
        timeout: Duration,
    ) -> Result<BashCommandOutput, Box<dyn Error + Send + Sync>>;

    /// Closes the session and cleanly terminates the microVM.
    pub async fn close(mut self) -> Result<(), Box<dyn Error + Send + Sync>>;
}

/// Session manager maintaining active microVM sessions across sequential invocations.
pub struct BashSessionManager {
    sessions: Arc<tokio::sync::Mutex<HashMap<String, BashSessionHandle>>>,
    vm_config: VmConfig,
    timeout: Duration,
}

impl BashSessionManager {
    pub fn new(vm_config: VmConfig, timeout: Duration) -> Self;

    /// Routes command to the active session for `session_id`, creating it if not present.
    pub async fn execute_in_session(
        &self,
        session_id: &str,
        command: &str,
    ) -> Result<BashCommandOutput, Box<dyn Error + Send + Sync>>;

    /// Explicitly closes and tears down a session.
    pub async fn close_session(
        &self,
        session_id: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;

    /// Cleans up sessions exceeding the inactivity timeout (125s).
    pub async fn cleanup_expired_sessions(&self);
}

/// Host-to-guest vsock TCP proxy forwarder for Chrome DevTools Protocol (CDP) (TOOL-BRW-003).
pub struct CdpVsockForwarder {
    pub host_bind_addr: SocketAddr,
    pub guest_port: u32,
    pub vsock_uds_path: PathBuf,
    shutdown_tx: tokio::sync::broadcast::Sender<()>,
    join_handle: tokio::task::JoinHandle<()>,
}

impl CdpVsockForwarder {
    /// Binds local TCP loopback port and proxies connections to Firecracker vsock port 9222.
    pub async fn start(
        vsock_uds_path: PathBuf,
        host_bind_addr: SocketAddr,
        guest_port: u32,
    ) -> Result<Self, Box<dyn Error + Send + Sync>>;

    pub fn stop(self);
    pub async fn wait(self) -> Result<(), tokio::task::JoinError>;
}
```

---

### 3.3 `aura-sdk/aura_sdk/tools/bash.py`
Updated to manage persistent sessions across sequential calls:
```python
import asyncio
import os
import uuid
from typing import Any
from aura_sdk.transport import ZenohClient


class BashTool:
    """
    Stateful Sandboxed Bash Tool (TOOL-BASH-001, TOOL-BASH-002).
    Preserves environment variables and working directory state across sequential invocations
    sharing the same session ID within an active task scope.
    """

    def __init__(
        self,
        transport: ZenohClient | None = None,
        default_session_id: str | None = None,
        timeout: float = 125.0,
    ):
        self.transport = transport
        self.default_session_id = default_session_id or str(uuid.uuid4())
        self.timeout = timeout
        self._local_sessions: dict[str, dict[str, Any]] = {}

    def get_session_id(self) -> str:
        return self.default_session_id

    def new_session(self) -> str:
        self.default_session_id = str(uuid.uuid4())
        return self.default_session_id

    async def execute(self, command: str, session_id: str | None = None) -> str:
        sid = session_id or self.default_session_id

        # Route through Zenoh bus when connected
        if self.transport is not None and not getattr(self.transport, "is_mock", True):
            payload = {"command": command, "session_id": sid}
            if hasattr(self.transport, "call_tool"):
                result = await self.transport.call_tool("bash", payload, timeout=self.timeout)
                if isinstance(result, dict):
                    return result.get("stdout") or result.get("result", "")
                return str(result)
            return await self.transport.dispatch_tool("bash", payload)

        # Standalone / mock stateful session execution
        session = self._local_sessions.setdefault(sid, {
            "cwd": os.getcwd(),
            "env": os.environ.copy(),
        })

        script = (
            f"cd {session['cwd']} 2>/dev/null || true\n"
            f"{command}\n"
            f"__AURA_EXIT__=$?\n"
            f"echo '__AURA_CWD_MARKER__'\n"
            f"pwd\n"
            f"echo '__AURA_ENV_MARKER__'\n"
            f"env\n"
        )

        proc = await asyncio.create_subprocess_shell(
            script,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
            env=session["env"],
        )
        stdout_bytes, stderr_bytes = await asyncio.wait_for(proc.communicate(), timeout=self.timeout)
        raw_out = stdout_bytes.decode("utf-8", errors="replace")

        if "__AURA_CWD_MARKER__" in raw_out:
            parts = raw_out.split("__AURA_CWD_MARKER__\n")
            user_output = parts[0]
            rest = parts[1]
            if "__AURA_ENV_MARKER__\n" in rest:
                cwd_part, env_part = rest.split("__AURA_ENV_MARKER__\n", 1)
                new_cwd = cwd_part.strip()
                if os.path.exists(new_cwd):
                    session["cwd"] = new_cwd
                for line in env_part.splitlines():
                    if "=" in line:
                        k, v = line.split("=", 1)
                        session["env"][k] = v
            return user_output.rstrip("\n")

        return raw_out
```

---

### 3.4 `aura-sdk/aura_sdk/tools/browser.py`
Updated with an asynchronous CDP client and complete browser automation suite:
```python
import asyncio
import base64
import json
import logging
from typing import Any

import httpx
import websockets

from aura_sdk.transport import ZenohClient

logger = logging.getLogger(__name__)


class AsyncCdpClient:
    """
    Asynchronous Chrome DevTools Protocol (CDP) client (TOOL-BRW-003).
    Communicates with headless Chromium via forwarded port 9222.
    """

    def __init__(self, cdp_url: str = "http://127.0.0.1:9222"):
        self.cdp_url = cdp_url.rstrip("/")
        self._ws_url: str | None = None
        self._ws: Any = None
        self._msg_id = 0
        self._target_id: str | None = None

    async def connect(self) -> None:
        async with httpx.AsyncClient() as client:
            resp = await client.get(f"{self.cdp_url}/json/new?about:blank", timeout=5.0)
            if resp.status_code == 200:
                data = resp.json()
                self._target_id = data.get("id")
                self._ws_url = data.get("webSocketDebuggerUrl")
            else:
                v_resp = await client.get(f"{self.cdp_url}/json/version", timeout=5.0)
                v_data = v_resp.json()
                self._ws_url = v_data.get("webSocketDebuggerUrl")

        if not self._ws_url:
            raise RuntimeError(f"Failed to obtain webSocketDebuggerUrl from {self.cdp_url}")

        self._ws = await websockets.connect(self._ws_url)

    async def send_command(self, method: str, params: dict[str, Any] | None = None) -> dict[str, Any]:
        if self._ws is None:
            await self.connect()

        self._msg_id += 1
        cmd_id = self._msg_id
        payload = {"id": cmd_id, "method": method, "params": params or {}}
        await self._ws.send(json.dumps(payload))

        while True:
            raw_msg = await self._ws.recv()
            msg = json.loads(raw_msg)
            if msg.get("id") == cmd_id:
                if "error" in msg:
                    raise RuntimeError(f"CDP Error in {method}: {msg['error']}")
                return msg.get("result", {})

    async def navigate(self, url: str) -> dict[str, Any]:
        return await self.send_command("Page.navigate", {"url": url})

    async def click(self, selector: str) -> dict[str, Any]:
        js = f"""
        (() => {{
            const el = document.querySelector({json.dumps(selector)});
            if (!el) throw new Error("Element not found: " + {json.dumps(selector)});
            el.click();
            return true;
        }})()
        """
        return await self.send_command("Runtime.evaluate", {"expression": js, "returnByValue": True})

    async def evaluate(self, script: str) -> Any:
        res = await self.send_command("Runtime.evaluate", {"expression": script, "returnByValue": True})
        return res.get("result", {}).get("value")

    async def capture_screenshot(self, format: str = "png") -> bytes:
        res = await self.send_command("Page.captureScreenshot", {"format": format})
        return base64.b64decode(res.get("data", ""))

    async def close(self) -> None:
        if self._ws is not None:
            await self._ws.close()
            self._ws = None


class BrowserTool:
    """
    Headless Chromium Browser Tool (TOOL-BRW-001, TOOL-BRW-002, TOOL-BRW-003).
    """

    def __init__(
        self,
        transport: ZenohClient | None = None,
        cdp_client: AsyncCdpClient | None = None,
        cdp_url: str = "http://127.0.0.1:9222",
    ):
        self.transport = transport
        self.cdp_url = cdp_url
        self._cdp_client = cdp_client

    async def _get_cdp(self) -> AsyncCdpClient:
        if self._cdp_client is None:
            self._cdp_client = AsyncCdpClient(self.cdp_url)
        return self._cdp_client

    async def navigate(self, url: str) -> dict[str, Any]:
        client = await self._get_cdp()
        try:
            res = await client.navigate(url)
            return {"status": "ok", "url": url, "result": res}
        except Exception:
            if self.transport:
                out = await self.transport.dispatch_tool("browser", {"action": "navigate", "url": url})
                return {"status": "ok", "url": url, "result": out}
            raise

    async def click(self, selector: str) -> dict[str, Any]:
        client = await self._get_cdp()
        try:
            res = await client.click(selector)
            return {"status": "ok", "selector": selector, "result": res}
        except Exception:
            if self.transport:
                out = await self.transport.dispatch_tool("browser", {"action": "click", "selector": selector})
                return {"status": "ok", "selector": selector, "result": out}
            raise

    async def evaluate(self, script: str) -> Any:
        client = await self._get_cdp()
        try:
            return await client.evaluate(script)
        except Exception:
            if self.transport:
                return await self.transport.dispatch_tool("browser", {"action": "evaluate", "script": script})
            raise

    async def screenshot(self, format: str = "png") -> bytes:
        client = await self._get_cdp()
        return await client.capture_screenshot(format=format)

    async def close(self) -> None:
        if self._cdp_client is not None:
            await self._cdp_client.close()
```

---

### 3.5 Documentation Updates (GAP-003)

#### In `aura-core/src/hypervisor/wasm.rs`
Add comprehensive module and function doc comments:
```rust
//! # WASI 0.3 WebAssembly Hypervisor (EXE-WASM-001 - 008, GAP-003)
//!
//! ## GAP-003: WASI 0.3 Component Model Dynamic Linking Limitation
//! Standard WASI 0.3 currently lacks standardized dynamic runtime hot-linking of newly
//! generated WebAssembly components into an existing process without pre-composition or host restart.
//! This is a known upstream Bytecode Alliance specification boundary.
//!
//! ### Architectural Mitigation (Static WIT Linking)
//! Aura enforces deploy-time static WIT component composition:
//! - Component interfaces (`aura:fs/read`, `aura:fs/write`, `wasi:cli/run@0.2.0`) are pre-compiled
//!   and statically linked into the `wasmtime::component::Linker` at host startup.
//! - Fuel metering and pooling instance allocators are enforced per invocation.
//!
//! ### Roadmap to WASI Component Model 1.0
//! When the Bytecode Alliance formally stabilizes the dynamic runtime component linking
//! specification in WASI Component Model 1.0, Aura will introduce a dynamic component registry
//! adapter to support on-the-fly component composition without host restart.
```

#### In `docs/architecture/adrs/ADR-001-Execution-Host.md`
Append Decision 5:
```markdown
5. **WASI 0.3 Dynamic Runtime Linking Boundary (GAP-003):**
   - **Context:** Standard WASI 0.3 lacks standardized dynamic runtime hot-linking of newly generated WebAssembly components without pre-composition or host restart.
   - **Decision:** Maintain deploy-time static WIT component linking with pre-registered ABI imports (`aura:fs`, `aura:graph`, `aura:tunnel`). Defer dynamic runtime linking until the Bytecode Alliance formally standardizes WASI Component Model 1.0 dynamic host linking.
```

#### In `.agents/plugins/aura-dev-kit/skills/aura-architecture/references/02-execution-sandboxes.md`
Update Section 1:
```markdown
- **Dynamic Linking Restriction (GAP-003)**: Standard WASI 0.3 lacks standardized dynamic runtime hot-linking of newly generated WASM components without pre-composition or host restart. Deploy-time static WIT component linking (via `wit-bindgen` and Cranelift compilation) must be utilized until Component Model 1.0 formally standardizes dynamic host linking.
```

---

## 4. Exact Files to Edit and Create

| File | Status | Action & Key Responsibility |
|---|---|---|
| `deploy/images/vsock-shell-daemon/Cargo.toml` | **Create** | Standalone guest binary package definition (`vsock`, `serde`, `serde_json`). |
| `deploy/images/vsock-shell-daemon/src/main.rs` | **Create** | CLI parser supporting `--listen --port 5250` and `--tcp-listen` test mode. |
| `deploy/images/vsock-shell-daemon/src/session.rs` | **Create** | Persistent child `/bin/bash` session manager with exit code & cwd markers. |
| `deploy/images/vsock-shell-daemon/src/server.rs` | **Create** | Vsock / TCP connection handler and JSON request/response framing. |
| `deploy/images/README.md` | **Edit** | Document `vsock-shell-daemon` specifications, vsock port 5250, and image setup. |
| `aura-core/src/hypervisor/firecracker.rs` | **Edit** | Add `DEFAULT_BASH_VSOCK_PORT`, `DEFAULT_CDP_VSOCK_PORT`, `BashCommandOutput`, `BashSessionHandle`, `BashSessionManager`, `CdpVsockForwarder`, and unit tests. |
| `aura-sdk/aura_sdk/tools/bash.py` | **Edit** | Enhance `BashTool` with session ID routing, sequential environment & working directory persistence, and session lifecycle methods. |
| `aura-sdk/aura_sdk/tools/browser.py` | **Edit** | Implement `AsyncCdpClient` and full `BrowserTool` automation methods (`navigate`, `click`, `evaluate`, `screenshot`, `close`). |
| `aura-sdk/tests/test_bash_tool.py` | **Create** | Pytest suite validating environment variable & cwd persistence across sequential bash calls for matching vs differing session IDs. |
| `aura-sdk/tests/test_browser_tool.py` | **Create** | Pytest suite with mock CDP HTTP/WebSocket server testing `navigate`, `click`, `evaluate`, and `screenshot`. |
| `aura-core/src/hypervisor/wasm.rs` | **Edit** | Add `GAP-003` module doc comments, function documentation, and static linking assertions. |
| `docs/architecture/adrs/ADR-001-Execution-Host.md` | **Edit** | Add Decision 5 detailing `GAP-003` static WIT mitigation and Component Model 1.0 roadmap. |
| `.agents/plugins/aura-dev-kit/skills/aura-architecture/references/02-execution-sandboxes.md` | **Edit** | Expand Section 1 documenting `GAP-003` dynamic linking restriction. |

---

## 5. Test Suite Design

### 5.1 Python SDK Bash Tool Tests (`aura-sdk/tests/test_bash_tool.py`)
```python
import pytest
from aura_sdk.tools.bash import BashTool


@pytest.mark.asyncio
async def test_bash_session_environment_persistence():
    bash = BashTool()
    session_id = "test-session-1"

    # Step 1: Export environment variable
    await bash.execute("export TEST_VAR=aura_session_ok", session_id=session_id)

    # Step 2: Read environment variable in subsequent call
    result = await bash.execute("echo $TEST_VAR", session_id=session_id)
    assert "aura_session_ok" in result


@pytest.mark.asyncio
async def test_bash_session_working_directory_persistence():
    bash = BashTool()
    session_id = "test-session-cwd"

    # Step 1: Change directory
    await bash.execute("cd /tmp", session_id=session_id)

    # Step 2: Query current directory in subsequent call
    result = await bash.execute("pwd", session_id=session_id)
    assert result.strip() == "/tmp"


@pytest.mark.asyncio
async def test_bash_session_isolation_between_sessions():
    bash = BashTool()
    session_a = "session-a"
    session_b = "session-b"

    await bash.execute("export SESSION_SECRET=secret_alpha", session_id=session_a)
    result_b = await bash.execute("echo $SESSION_SECRET", session_id=session_b)
    assert "secret_alpha" not in result_b
```

### 5.2 Python SDK Browser Tool Tests (`aura-sdk/tests/test_browser_tool.py`)
```python
import asyncio
import base64
import json
import pytest
from aiohttp import web
import websockets
from aura_sdk.tools.browser import AsyncCdpClient, BrowserTool

PNG_MAGIC_BYTES = b"\x89PNG\r\n\x1a\n"


@pytest.mark.asyncio
async def test_browser_cdp_actions_against_mock_cdp_server():
    # 1. Spin up mock CDP HTTP & WebSocket server
    ...
    # 2. Verify navigate, evaluate, click, screenshot return structured results
    browser = BrowserTool(cdp_url=f"http://127.0.0.1:{mock_port}")
    nav_res = await browser.navigate("https://aura.local")
    assert nav_res["status"] == "ok"

    eval_res = await browser.evaluate("2 + 2")
    assert eval_res == 4

    click_res = await browser.click("#submit")
    assert click_res["status"] == "ok"

    shot = await browser.screenshot()
    assert shot.startswith(PNG_MAGIC_BYTES)

    await browser.close()
```

### 5.3 Rust Core Tests (`aura-core/src/hypervisor/firecracker.rs`)
1. `test_bash_session_handle_creation_and_config`: validates `BashSessionHandle` configuration, timeout, and rootfs validation.
2. `test_bash_session_manager_routing`: verifies that sequential invocations with the same `session_id` route to the same cached session instance.
3. `test_cdp_vsock_forwarder_bind`: validates that `CdpVsockForwarder` binds the designated loopback address, creates the Unix domain socket client task, and terminates on `stop()`.

---

## 6. Step-by-Step Implementation Plan

### Step 1: Guest-Side Shell Daemon (`deploy/images/vsock-shell-daemon`)
1. Create `deploy/images/vsock-shell-daemon/Cargo.toml`.
2. Implement `src/session.rs` with persistent `/bin/bash` subprocess management, exit code detection, and `cwd` tracking.
3. Implement `src/server.rs` with `AF_VSOCK` listener and TCP fallback for testing.
4. Implement `src/main.rs` with CLI option parsing.
5. Update `deploy/images/README.md` documenting the daemon and `bash.ext4` setup.

### Step 2: Rust Hypervisor Session Management & Forwarder (`firecracker.rs`)
1. Add `DEFAULT_BASH_VSOCK_PORT` (5250) and `DEFAULT_CDP_VSOCK_PORT` (9222).
2. Implement `BashCommandOutput`, `BashSessionRequest`, and `BashSessionHandle`.
3. Implement `BashSessionManager` with thread-safe session caching and inactivity expiration.
4. Implement `CdpVsockForwarder` proxying TCP loopback traffic to `vsock_uds_path`.
5. Add unit tests for `BashSessionHandle`, `BashSessionManager`, and `CdpVsockForwarder`.

### Step 3: Python SDK Bash Tool Persistence (`bash.py`)
1. Update `BashTool` in `aura-sdk/aura_sdk/tools/bash.py` to support `session_id`.
2. Implement stateful session tracking for standalone and mock execution.
3. Ensure Zenoh dispatch passes `session_id` in the tool execution payload.
4. Add `aura-sdk/tests/test_bash_tool.py` and verify with pytest.

### Step 4: Python SDK Browser Tool CDP Driver (`browser.py`)
1. Implement `AsyncCdpClient` with HTTP discovery and WebSocket command exchange using `httpx` and `websockets`.
2. Implement `BrowserTool` methods: `navigate(url)`, `click(selector)`, `evaluate(script)`, `screenshot()`, and `close()`.
3. Add `aura-sdk/tests/test_browser_tool.py` testing all CDP actions against a mock CDP endpoint.
4. Verify all tests pass with `pytest aura-sdk/tests`.

### Step 5: Architecture Documentation & GAP-003 Acknowledgment
1. Add GAP-003 module-level doc comments and static WIT component linking documentation to `aura-core/src/hypervisor/wasm.rs`.
2. Update `docs/architecture/adrs/ADR-001-Execution-Host.md` adding Decision 5 for GAP-003.
3. Update `.agents/plugins/aura-dev-kit/skills/aura-architecture/references/02-execution-sandboxes.md` detailing the Component Model 1.0 path forward.

---

## 7. Caveats

- **Hardware KVM Dependencies in Guest Testing**: In unit test environments without hardware `/dev/kvm` and root loop-device permissions, `FirecrackerVm::configure_and_boot` uses non-blocking test fallbacks. The guest daemons (`vsock-shell-daemon`, `vsock-screenshot-daemon`) provide TCP fallback listeners (`--tcp-listen`) to enable 100% test coverage without live KVM virtualization.
- **Upstream Bytecode Alliance Evolution**: WASI Component Model 1.0 dynamic runtime linking remains under active development by the Bytecode Alliance. Maintaining static WIT linking is the only standards-compliant approach until Component Model 1.0 reaches general availability.
- **Browser Memory Footprint**: Headless Chromium requires a minimum of 1024 MiB RAM per microVM instance (`VmConfig::for_browser`). Simultaneous multi-session browser testing should avoid launching large numbers of concurrent VM instances without adequate host RAM.

---

## 8. Conclusion

All three requirements (`TOOL-BASH-002`, `TOOL-BRW-003`, `GAP-003`) are completely specified and actionable:
1. `TOOL-BASH-002` bridges the gap from ephemeral one-shot commands to persistent multi-call shell sessions via a dedicated guest daemon, host session handle, and SDK session routing.
2. `TOOL-BRW-003` bridges host CDP clients to guest Chromium over AF_VSOCK forwarding, providing a complete, async Python browser automation tool without bloated external dependencies.
3. `GAP-003` formally documents the WASI 0.3 specification limitation and solidifies the static WIT linking mitigation and roadmap to Component Model 1.0 across codebase doc comments and architecture references.

---

## 9. Verification Method

To independently verify the implementation:
1. **Python SDK Test Suite**:
   ```bash
   /home/jeff/aura/aura-sdk/.venv/bin/pytest aura-sdk/tests/test_bash_tool.py aura-sdk/tests/test_browser_tool.py
   /home/jeff/aura/aura-sdk/.venv/bin/pytest aura-sdk/tests
   ```
   Must assert:
   - Bash environment variables and working directory persist across sequential calls with the same session ID.
   - Different session IDs are isolated.
   - Browser tool CDP actions (`navigate`, `evaluate`, `click`, `screenshot`) succeed and return structured responses.
2. **Rust Microkernel Test Suite**:
   ```bash
   cargo test -p aura-core -- hypervisor
   ```
   Must assert:
   - Unit tests for `BashSessionHandle`, `BashSessionManager`, and `CdpVsockForwarder` compile and pass.
   - WASM static component tests compile and pass.
3. **Architecture Documentation Review**:
   - Inspect `aura-core/src/hypervisor/wasm.rs`, `ADR-001-Execution-Host.md`, and `references/02-execution-sandboxes.md` to confirm GAP-003 comments and mitigations are present.
