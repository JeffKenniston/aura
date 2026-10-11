# Forensic Investigation & Remediation Strategy: BrowserTool & WebSocket Resilience

**Agent ID**: `explorer_m4_fix_2`  
**Working Directory**: `/home/jeff/aura/.agents/teamwork/explorer_m4_fix_2/`  
**Parent Agent ID**: `ad091f9f-df8d-4332-bf82-a7111f5328d2`  
**Target Milestone**: Milestone 4: Persistent Stateful Tools (`TOOL-BRW-003`)  
**Scope**: Remediating `aura-sdk/aura_sdk/tools/browser.py` and `aura-sdk/tests/test_browser_tool_stress.py`

---

## 1. Observation

### 1.1 Verified File Paths & Line Numbers
Investigation of `aura-sdk/aura_sdk/tools/browser.py` revealed three critical implementation defects:

1. **Unbounded Screenshot Frame Truncation (`browser.py:76`)**:
   ```python
   76:         self._ws = await websockets.connect(self._ws_url)
   ```
   `websockets.connect` defaults to `max_size=1048576` (1 MiB). When `BrowserTool.screenshot()` captures screenshots of high-resolution viewports (e.g. 1080p, 4K, or full page), the base64-encoded JSON-RPC response exceeds 1 MiB. The WebSocket client immediately raises:
   ```text
   websockets.exceptions.ConnectionClosedError: sent 1009 (message too big) frame exceeds limit of 1048576 bytes; no close frame received
   ```

2. **Cold-Start Connection Race Condition (`browser.py:82-86`)**:
   ```python
   82:         if self._ws is None:
   83:             await self.connect()
   84: 
   85:         async with self._lock:
   86:             self._msg_id += 1
   ```
   `if self._ws is None: await self.connect()` executes **outside** `self._lock`. When multiple concurrent asyncio tasks invoke a fresh or disconnected `BrowserTool`, all tasks observe `self._ws is None` and execute HTTP discovery (`/json/new`, `/json/list`, `/json/version`) and `websockets.connect()` simultaneously. Each task overrides `self._ws` with a different WebSocket connection handle, leaving earlier tasks waiting for responses on abandoned or replaced sockets, leading to dropped messages and:
   ```text
   asyncio.TimeoutError: Timeout waiting for CDP command response
   ```

3. **Stale WebSocket Handle on Disconnect Preventing Auto-Recovery (`browser.py:40-41, 88-108`)**:
   ```python
   40:     @property
   41:     def is_connected(self) -> bool:
   42:         return self._ws is not None
   ```
   When Chromium drops a connection, restarts, or virtio-vsock closes the stream, `self._ws.send` or `self._ws.recv` raises `websockets.exceptions.ConnectionClosed`. However, `self._ws` is **never reset to `None`** in exception handlers. On subsequent calls:
   `self.is_connected` evaluates to `True`, so `self.connect()` is skipped. The client attempts `await self._ws.send(...)` on the dead socket, throwing repeated `ConnectionClosed` errors and permanently disabling the tool until manual `tool.close()` is called.

### 1.2 Test Suite State & Defect-Proof Tests
In `aura-sdk/tests/test_browser_tool_stress.py`, challenger agent `challenger_m4_2` authored 11 empirical stress tests. Three of these tests were written as "defect proofs" asserting that the bugs exist:
- `test_screenshot_over_1mb_fails_due_to_websockets_default_max_size` (lines 312-329): asserts `ConnectionClosedError` 1009.
- `test_cold_client_concurrent_connect_race_condition` (lines 351-368): asserts `TimeoutError`.
- `test_server_disconnect_leaves_stale_ws_handle` (lines 422-449): asserts `ConnectionClosed` on subsequent call.

Once `browser.py` is fixed, these three tests must be updated to assert **resilient success** (`test_screenshot_over_1mb_succeeds`, `test_cold_client_concurrent_connect_synchronized`, `test_server_disconnect_auto_reconnects`).

---

## 2. Logic Chain

1. **Frame Size Limit (Observation 1.1.1)**:
   - Modern browser viewports encoded as PNG/JPEG regularly reach 1.5MB to 4MB in raw binary, and ~2MB to ~5.3MB in base64 string representation.
   - Setting `max_size=32 * 1024 * 1024` (32 MiB) on `websockets.connect` guarantees sufficient headroom for 4K desktop captures while enforcing an upper bound to prevent host memory exhaustion.

2. **Connection Synchronization & Re-entrancy (Observation 1.1.2)**:
   - In Python `asyncio`, `asyncio.Lock` is **non-reentrant**. Naively wrapping `connect()` inside `self._lock` and calling `await self.connect()` while holding `self._lock` causes immediate, permanent deadlock.
   - The correct architectural pattern is:
     - Provide an internal routine `_connect_locked(self) -> None` that is guaranteed to run while `self._lock` is held.
     - Public `connect()` acquires `self._lock` and delegates to `_connect_locked()`.
     - `send_command()` acquires `self._lock`, checks `if not self.is_connected: await self._connect_locked()`, and then safely issues the command.
   - This ensures that 100% of concurrent cold calls serialize on the same lock: the first task performs target discovery and establishes `self._ws`; all subsequent tasks find `self.is_connected == True`, reuse the established connection, and proceed sequentially without race conditions.

3. **Lifecycle State Management & Auto-Reconnection (Observation 1.1.3)**:
   - In `AsyncCdpClient`:
     ```python
     @property
     def is_connected(self) -> bool:
         if self._ws is None:
             return False
         state = getattr(self._ws, "state", None)
         if state is not None:
             return state == websockets.protocol.State.OPEN
         return True
     ```
   - In `send_command`: wrap the WebSocket transaction in `try ... except (websockets.exceptions.ConnectionClosed, ConnectionResetError, BrokenPipeError): self._ws = None; raise`.
   - In `close()`: wrap in `async with self._lock:` and ensure `self._ws = None`.
   - When a connection drops mid-flight, `self._ws` is immediately cleared to `None`. On the next command, `not self.is_connected` evaluates to `True`, triggering transparent, automatic reconnection.

4. **Empirical Validation**:
   - We verified this logic in `test_remediation.py` and `proposed_test_browser_tool_stress.py`.
   - All 10/10 scratch remediation tests and all 11/11 stress tests passed with 0 errors in under 0.6 seconds.

---

## 3. Caveats

1. **Bash Tool Failures**:
   - `pytest aura-sdk/tests` currently reports 3 failures in `test_bash_stress.py` (`test_adversarial_space_in_working_directory`, `test_adversarial_unset_environment_variable`, `test_adversarial_stderr_capture_on_command_failure`).
   - These are isolated to `aura-sdk/aura_sdk/tools/bash.py` and are tracked by peer remediation worker `explorer_m4_fix_1`.
2. **Stress Test File Mutation Requirement**:
   - Because `challenger_m4_2` wrote tests 5, 7, and 11 in `test_browser_tool_stress.py` to assert that the bug occurs (`pytest.raises`), applying the fix to `browser.py` requires applying `test_stress_remediation.patch` to update those tests to assert success.
3. **No Mocks or Stubs**:
   - In compliance with `AGENTS.md`, zero stubs, mocks, or TODOs have been used. Both the remediation code and test suites use real `websockets.asyncio.server.serve` loopback connections and authentic JSON-RPC CDP messages.

---

## 4. Conclusion

The remediation strategy completely resolves all three vulnerabilities identified in `TOOL-BRW-003`.

### 4.1 Artifacts Generated in Working Directory
The following drop-in artifacts have been authored and verified in `/home/jeff/aura/.agents/teamwork/explorer_m4_fix_2/`:
1. `proposed_browser.py`: Complete drop-in source replacement for `aura-sdk/aura_sdk/tools/browser.py`.
2. `browser_remediation.patch`: Exact unified diff patch for `aura-sdk/aura_sdk/tools/browser.py`.
3. `proposed_test_browser_tool_stress.py`: Complete drop-in source replacement for `aura-sdk/tests/test_browser_tool_stress.py`.
4. `test_stress_remediation.patch`: Exact unified diff patch for `aura-sdk/tests/test_browser_tool_stress.py`.
5. `test_remediation.py`: Standalone verification test harness.

### 4.2 Drop-In Remediation Code: `aura-sdk/aura_sdk/tools/browser.py`

```python
"""
Aura Headless Chromium Browser Tool (TOOL-BRW-001, TOOL-BRW-002, TOOL-BRW-003).

Provides an asynchronous Chrome DevTools Protocol (CDP) client connecting
to headless Chromium running inside a Firecracker microVM via vsock TCP forwarding.
Supports navigation, DOM element clicking, JavaScript evaluation, and PNG screenshots.
"""

from __future__ import annotations

import asyncio
import base64
import json
import logging
from typing import Any

import httpx
import websockets

from aura_sdk.transport import ZenohClient

logger = logging.getLogger(__name__)

DEFAULT_MAX_WS_PAYLOAD_SIZE = 32 * 1024 * 1024  # 32 MiB for large screenshots


class AsyncCdpClient:
    """
    Asynchronous Chrome DevTools Protocol (CDP) client (TOOL-BRW-003).
    Communicates with headless Chromium via forwarded port 9222.
    """

    def __init__(
        self,
        cdp_url: str = "http://127.0.0.1:9222",
        max_size: int | None = DEFAULT_MAX_WS_PAYLOAD_SIZE,
    ):
        self.cdp_url = cdp_url.rstrip("/")
        self.max_size = max_size
        self._ws_url: str | None = None
        self._ws: Any = None
        self._msg_id: int = 0
        self._target_id: str | None = None
        self._lock = asyncio.Lock()

    @property
    def is_connected(self) -> bool:
        """Indicates whether an active WebSocket connection is open."""
        if self._ws is None:
            return False
        state = getattr(self._ws, "state", None)
        if state is not None:
            return state == websockets.protocol.State.OPEN
        return True

    async def connect(self) -> None:
        """Discovers the page/browser target and opens a WebSocket connection."""
        async with self._lock:
            await self._connect_locked()

    async def _connect_locked(self) -> None:
        """
        Internal connection routine.
        MUST only be invoked while holding self._lock to ensure thread-safety
        and prevent race conditions on cold starts.
        """
        if self.is_connected:
            return

        # Ensure any existing/stale connection handle is cleaned up
        if self._ws is not None:
            try:
                await self._ws.close()
            except Exception:
                pass
            self._ws = None

        async with httpx.AsyncClient() as client:
            target_data: dict[str, Any] | None = None
            try:
                resp = await client.get(f"{self.cdp_url}/json/new?about:blank", timeout=5.0)
                if resp.status_code == 200:
                    target_data = resp.json()
            except Exception:
                target_data = None

            if not target_data or not target_data.get("webSocketDebuggerUrl"):
                try:
                    resp = await client.get(f"{self.cdp_url}/json/list", timeout=5.0)
                    if resp.status_code == 200:
                        targets = resp.json()
                        if isinstance(targets, list) and targets:
                            page_targets = [t for t in targets if t.get("type") == "page"]
                            target_data = page_targets[0] if page_targets else targets[0]
                except Exception:
                    pass

            if target_data and target_data.get("webSocketDebuggerUrl"):
                self._target_id = target_data.get("id")
                self._ws_url = target_data.get("webSocketDebuggerUrl")
            else:
                v_resp = await client.get(f"{self.cdp_url}/json/version", timeout=5.0)
                v_data = v_resp.json()
                self._ws_url = v_data.get("webSocketDebuggerUrl")

        if not self._ws_url:
            raise RuntimeError(f"Failed to obtain webSocketDebuggerUrl from {self.cdp_url}")

        try:
            self._ws = await websockets.connect(self._ws_url, max_size=self.max_size)
        except Exception:
            self._ws = None
            raise

    async def send_command(
        self, method: str, params: dict[str, Any] | None = None, timeout: float = 10.0
    ) -> dict[str, Any]:
        """Sends a JSON-RPC CDP command and awaits its corresponding result."""
        async with self._lock:
            if not self.is_connected:
                await self._connect_locked()

            self._msg_id += 1
            cmd_id = self._msg_id
            payload = {"id": cmd_id, "method": method, "params": params or {}}

            try:
                await self._ws.send(json.dumps(payload))

                async def _recv_response() -> dict[str, Any]:
                    while True:
                        raw_msg = await self._ws.recv()
                        if isinstance(raw_msg, bytes):
                            raw_msg = raw_msg.decode("utf-8")
                        msg = json.loads(raw_msg)
                        if msg.get("id") == cmd_id:
                            if "error" in msg:
                                err = msg["error"]
                                err_msg = (
                                    err.get("message", str(err))
                                    if isinstance(err, dict)
                                    else str(err)
                                )
                                raise RuntimeError(f"CDP command '{method}' error: {err_msg}")
                            return msg.get("result", {})

                return await asyncio.wait_for(_recv_response(), timeout=timeout)
            except (websockets.exceptions.ConnectionClosed, ConnectionResetError, BrokenPipeError):
                self._ws = None
                raise
            except Exception:
                if self._ws is not None and not self.is_connected:
                    self._ws = None
                raise

    async def navigate(self, url: str) -> dict[str, Any]:
        """Navigates the page to the target URL."""
        return await self.send_command("Page.navigate", {"url": url})

    async def click(self, selector: str) -> dict[str, Any]:
        """Clicks an element matching the given CSS selector."""
        js = f"""
        (() => {{
            const el = document.querySelector({json.dumps(selector)});
            if (!el) throw new Error("Element not found: " + {json.dumps(selector)});
            el.click();
            return true;
        }})()
        """
        res = await self.send_command(
            "Runtime.evaluate", {"expression": js, "returnByValue": True, "awaitPromise": True}
        )
        if "exceptionDetails" in res:
            exc = res["exceptionDetails"]
            desc = exc.get("text", "")
            if "exception" in exc and isinstance(exc["exception"], dict):
                desc = exc["exception"].get("description", desc)
            raise RuntimeError(f"Click failed: {desc}")
        return res.get("result", {})

    async def evaluate(self, expression: str) -> Any:
        """Evaluates JavaScript expression in page context and returns value."""
        res = await self.send_command(
            "Runtime.evaluate",
            {"expression": expression, "returnByValue": True, "awaitPromise": True},
        )
        if "exceptionDetails" in res:
            exc = res["exceptionDetails"]
            desc = exc.get("text", "")
            if "exception" in exc and isinstance(exc["exception"], dict):
                desc = exc["exception"].get("description", desc)
            raise RuntimeError(f"Evaluation error: {desc}")
        return res.get("result", {}).get("value")

    async def capture_screenshot(self, format: str = "png") -> bytes:
        """Captures page screenshot and returns raw bytes."""
        res = await self.send_command("Page.captureScreenshot", {"format": format})
        raw_b64 = res.get("data", "")
        return base64.b64decode(raw_b64)

    async def close(self) -> None:
        """Closes the WebSocket connection."""
        async with self._lock:
            if self._ws is not None:
                try:
                    await self._ws.close()
                except Exception:
                    pass
                self._ws = None


class BrowserTool:
    """
    Headless Chromium Browser Tool (TOOL-BRW-001, TOOL-BRW-002, TOOL-BRW-003).
    Controls headless Chromium over Chrome DevTools Protocol (CDP) via Firecracker vsock forwarder.
    """

    def __init__(
        self,
        transport: ZenohClient | None = None,
        cdp_client: AsyncCdpClient | None = None,
        cdp_url: str = "http://127.0.0.1:9222",
        max_size: int | None = DEFAULT_MAX_WS_PAYLOAD_SIZE,
    ):
        self.transport = transport
        self.cdp_url = cdp_url
        self.max_size = max_size
        self._cdp_client = cdp_client

    async def _get_cdp(self) -> AsyncCdpClient:
        if self._cdp_client is None:
            self._cdp_client = AsyncCdpClient(self.cdp_url, max_size=self.max_size)
        return self._cdp_client

    async def navigate(self, url: str) -> dict[str, Any]:
        """Navigates to URL and returns status with frame metadata."""
        client = await self._get_cdp()
        try:
            res = await client.navigate(url)
            return {"status": "ok", "url": url, "result": res}
        except Exception:
            if self.transport is not None and not getattr(self.transport, "is_mock", False):
                out = await self.transport.dispatch_tool("browser", {"action": "navigate", "url": url})
                return {"status": "ok", "url": url, "result": out}
            raise

    async def click(self, selector: str) -> dict[str, Any]:
        """Clicks element matching CSS selector."""
        client = await self._get_cdp()
        try:
            res = await client.click(selector)
            return {"status": "ok", "selector": selector, "result": res}
        except Exception:
            if self.transport is not None and not getattr(self.transport, "is_mock", False):
                out = await self.transport.dispatch_tool("browser", {"action": "click", "selector": selector})
                return {"status": "ok", "selector": selector, "result": out}
            raise

    async def evaluate(self, script: str) -> Any:
        """Evaluates JavaScript expression in page context."""
        client = await self._get_cdp()
        try:
            return await client.evaluate(script)
        except Exception:
            if self.transport is not None and not getattr(self.transport, "is_mock", False):
                return await self.transport.dispatch_tool("browser", {"action": "evaluate", "script": script})
            raise

    async def screenshot(self, format: str = "png") -> bytes:
        """Captures page screenshot as raw PNG/JPEG bytes."""
        client = await self._get_cdp()
        return await client.capture_screenshot(format=format)

    async def close(self) -> None:
        """Closes the underlying CDP client."""
        if self._cdp_client is not None:
            await self._cdp_client.close()
```

---

## 5. Verification Method

### 5.1 Verification Commands
The implementing worker can apply the patches and verify the fix using these commands:

```bash
# 1. Apply remediation patch to browser.py
patch -p1 < /home/jeff/aura/.agents/teamwork/explorer_m4_fix_2/browser_remediation.patch

# 2. Apply stress test patch to test_browser_tool_stress.py
patch -p1 < /home/jeff/aura/.agents/teamwork/explorer_m4_fix_2/test_stress_remediation.patch

# 3. Activate venv and run all browser tests
cd /home/jeff/aura
source aura-sdk/.venv/bin/activate
pytest -v aura-sdk/tests/test_browser_tool.py aura-sdk/tests/test_browser_tool_stress.py
```

### 5.2 Expected Verification Results
```text
aura-sdk/tests/test_browser_tool.py::test_async_cdp_client_navigation PASSED
aura-sdk/tests/test_browser_tool.py::test_browser_tool_navigate_and_evaluate PASSED
aura-sdk/tests/test_browser_tool.py::test_browser_tool_click_success_and_failure PASSED
aura-sdk/tests/test_browser_tool.py::test_browser_tool_screenshot PASSED
aura-sdk/tests/test_browser_tool.py::test_browser_tool_syntax_error_evaluation PASSED
aura-sdk/tests/test_browser_tool.py::test_zero_trust_unreachable_endpoint PASSED
aura-sdk/tests/test_browser_tool_stress.py::test_unsolicited_events_interleaving_and_deserialization PASSED
aura-sdk/tests/test_browser_tool_stress.py::test_invalid_selectors_syntax_and_missing PASSED
aura-sdk/tests/test_browser_tool_stress.py::test_complex_evaluations_and_exceptions PASSED
aura-sdk/tests/test_browser_tool_stress.py::test_screenshot_under_1mb_succeeds PASSED
aura-sdk/tests/test_browser_tool_stress.py::test_screenshot_over_1mb_succeeds PASSED
aura-sdk/tests/test_browser_tool_stress.py::test_high_concurrency_serialization_when_connected PASSED
aura-sdk/tests/test_browser_tool_stress.py::test_cold_client_concurrent_connect_synchronized PASSED
aura-sdk/tests/test_browser_tool_stress.py::test_network_timeout_and_out_of_order_recovery PASSED
aura-sdk/tests/test_browser_tool_stress.py::test_target_discovery_fallbacks PASSED
aura-sdk/tests/test_browser_tool_stress.py::test_close_and_reconnection_lifecycle PASSED
aura-sdk/tests/test_browser_tool_stress.py::test_server_disconnect_auto_reconnects PASSED

======================== 17 passed, 1 warning in 0.57s =========================
```

### 5.3 Invalidation Conditions
This remediation strategy is invalidated if:
1. `AsyncCdpClient` deadlocks when `connect()` and `send_command()` are invoked concurrently. (Disproved: tested with 10 concurrent cold calls).
2. Screenshots exceeding 1 MiB raise `ConnectionClosedError` 1009. (Disproved: tested up to 3.5 MiB).
3. Subsequent commands fail after server recovery without calling `tool.close()`. (Disproved: tested with forced server disconnect and auto-recovery).
