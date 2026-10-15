"""
Unit and integration tests for BrowserTool and AsyncCdpClient (TOOL-BRW-003).
Verifies HTTP target discovery, WebSocket JSON-RPC communication,
navigation, element clicking, JavaScript evaluation, screenshot capturing,
and error handling.
"""

from __future__ import annotations

import base64
import json

import httpx
import pytest
from websockets.asyncio.server import serve
from websockets.datastructures import Headers
from websockets.http11 import Response

from aura_agents.tools.browser import AsyncCdpClient, BrowserTool

PNG_MAGIC_BYTES = b"\x89PNG\r\n\x1a\n"


@pytest.fixture
async def cdp_emulator():
    """
    Spawns an in-process genuine HTTP + WebSocket CDP emulator server
    on an ephemeral loopback port for authentic network testing.
    """
    state = {
        "navigated_urls": [],
        "clicked_selectors": [],
        "evaluated_expressions": [],
    }

    async def process_request(connection, request):
        port = connection.local_address[1]
        if request.path.startswith("/json/new") or request.path.startswith("/json/version"):
            body = json.dumps({
                "id": "target-page-01",
                "type": "page",
                "title": "Aura Browser Sandbox",
                "url": "about:blank",
                "webSocketDebuggerUrl": f"ws://127.0.0.1:{port}/devtools/page/target-page-01",
            }).encode()
            headers = Headers([
                ("Content-Type", "application/json"),
                ("Content-Length", str(len(body))),
            ])
            return Response(200, "OK", headers, body)
        elif request.path.startswith("/json/list"):
            body = json.dumps([{
                "id": "target-page-01",
                "type": "page",
                "title": "Aura Browser Sandbox",
                "url": "about:blank",
                "webSocketDebuggerUrl": f"ws://127.0.0.1:{port}/devtools/page/target-page-01",
            }]).encode()
            headers = Headers([
                ("Content-Type", "application/json"),
                ("Content-Length", str(len(body))),
            ])
            return Response(200, "OK", headers, body)
        return None

    async def ws_handler(websocket):
        async for raw_msg in websocket:
            msg = json.loads(raw_msg)
            cmd_id = msg.get("id")
            method = msg.get("method")
            params = msg.get("params", {})

            if method == "Page.navigate":
                url = params.get("url", "")
                state["navigated_urls"].append(url)
                resp = {
                    "id": cmd_id,
                    "result": {"frameId": "frame-uuid-123", "loaderId": "loader-uuid-456"},
                }
                await websocket.send(json.dumps(resp))

            elif method == "Runtime.evaluate":
                expr = params.get("expression", "")
                state["evaluated_expressions"].append(expr)
                if "nonexistent" in expr:
                    resp = {
                        "id": cmd_id,
                        "result": {
                            "exceptionDetails": {
                                "text": "Error: Element not found: #nonexistent",
                                "exception": {"description": "Error: Element not found: #nonexistent"},
                            }
                        },
                    }
                elif "SYNTAX_ERROR" in expr:
                    resp = {
                        "id": cmd_id,
                        "result": {
                            "exceptionDetails": {
                                "text": "SyntaxError: Unexpected token",
                            }
                        },
                    }
                elif "document.title" in expr:
                    resp = {
                        "id": cmd_id,
                        "result": {"result": {"type": "string", "value": "Aura Documentation"}},
                    }
                elif "1 + 1" in expr:
                    resp = {
                        "id": cmd_id,
                        "result": {"result": {"type": "number", "value": 2}},
                    }
                elif "click()" in expr:
                    state["clicked_selectors"].append(expr)
                    resp = {
                        "id": cmd_id,
                        "result": {"result": {"type": "boolean", "value": True}},
                    }
                else:
                    resp = {
                        "id": cmd_id,
                        "result": {"result": {"type": "string", "value": "default_eval"}},
                    }
                await websocket.send(json.dumps(resp))

            elif method == "Page.captureScreenshot":
                sample_png = PNG_MAGIC_BYTES + b"IMAGE_STREAM_TEST_PAYLOAD"
                b64_data = base64.b64encode(sample_png).decode("utf-8")
                resp = {"id": cmd_id, "result": {"data": b64_data}}
                await websocket.send(json.dumps(resp))

            else:
                resp = {"id": cmd_id, "result": {}}
                await websocket.send(json.dumps(resp))

    server = await serve(ws_handler, "127.0.0.1", 0, process_request=process_request)
    port = server.sockets[0].getsockname()[1]
    cdp_url = f"http://127.0.0.1:{port}"

    yield {"cdp_url": cdp_url, "port": port, "state": state}

    server.close()
    await server.wait_closed()


@pytest.mark.asyncio
async def test_async_cdp_client_navigation(cdp_emulator):
    client = AsyncCdpClient(cdp_emulator["cdp_url"])
    res = await client.navigate("https://aura.local/welcome")
    assert "frameId" in res
    assert res["frameId"] == "frame-uuid-123"
    assert "https://aura.local/welcome" in cdp_emulator["state"]["navigated_urls"]
    await client.close()


@pytest.mark.asyncio
async def test_browser_tool_navigate_and_evaluate(cdp_emulator):
    tool = BrowserTool(cdp_url=cdp_emulator["cdp_url"])
    nav_res = await tool.navigate("https://aura.local/dashboard")
    assert nav_res["status"] == "ok"
    assert nav_res["url"] == "https://aura.local/dashboard"
    assert nav_res["result"]["frameId"] == "frame-uuid-123"

    title = await tool.evaluate("document.title")
    assert title == "Aura Documentation"

    math_val = await tool.evaluate("1 + 1")
    assert math_val == 2

    await tool.close()


@pytest.mark.asyncio
async def test_browser_tool_click_success_and_failure(cdp_emulator):
    tool = BrowserTool(cdp_url=cdp_emulator["cdp_url"])
    click_res = await tool.click("#submit-button")
    assert click_res["status"] == "ok"
    assert click_res["selector"] == "#submit-button"

    with pytest.raises(RuntimeError) as exc_info:
        await tool.click("#nonexistent")
    assert "Element not found" in str(exc_info.value)

    await tool.close()


@pytest.mark.asyncio
async def test_browser_tool_screenshot(cdp_emulator):
    tool = BrowserTool(cdp_url=cdp_emulator["cdp_url"])
    png_bytes = await tool.screenshot(format="png")
    assert isinstance(png_bytes, bytes)
    assert png_bytes.startswith(PNG_MAGIC_BYTES)
    assert b"IMAGE_STREAM_TEST_PAYLOAD" in png_bytes
    await tool.close()


@pytest.mark.asyncio
async def test_browser_tool_syntax_error_evaluation(cdp_emulator):
    tool = BrowserTool(cdp_url=cdp_emulator["cdp_url"])
    with pytest.raises(RuntimeError) as exc_info:
        await tool.evaluate("SYNTAX_ERROR(;;)")
    assert "SyntaxError" in str(exc_info.value)
    await tool.close()


@pytest.mark.asyncio
async def test_zero_trust_unreachable_endpoint():
    tool = BrowserTool(cdp_url="http://127.0.0.1:49999")
    with pytest.raises((httpx.HTTPError, RuntimeError, OSError)):
        await tool.navigate("https://aura.local")
