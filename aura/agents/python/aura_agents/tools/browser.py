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

from aura_agents.transport import ZenohClient

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
