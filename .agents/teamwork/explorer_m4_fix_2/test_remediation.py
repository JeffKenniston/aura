"""
Comprehensive verification test for proposed_browser.py in explorer_m4_fix_2.
Tests both baseline functionality and adversarial stress resilience:
1. Baseline navigation, click, evaluate, screenshot, syntax error, unreachable endpoint.
2. Handling screenshots > 1MB (e.g. 2.5MB payload).
3. Elimination of cold-start connection race conditions (10 concurrent cold requests).
4. Automatic reconnection on subsequent calls after server disconnect / connection drop.
"""

from __future__ import annotations

import asyncio
import base64
import json
import os
import sys
import pytest
import websockets
from websockets.asyncio.server import serve
from websockets.datastructures import Headers
from websockets.http11 import Response

# Import proposed implementation
sys.path.insert(0, "/home/jeff/aura/.agents/teamwork/explorer_m4_fix_2")
from proposed_browser import AsyncCdpClient, BrowserTool

PNG_MAGIC_BYTES = b"\x89PNG\r\n\x1a\n"


@pytest.fixture
async def baseline_cdp_emulator():
    """
    CDP emulator matching test_browser_tool.py.
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


@pytest.fixture
async def stress_cdp_server():
    server_control = {
        "delay_seconds": 0.0,
        "inject_unsolicited_events": True,
        "force_disconnect": False,
        "new_endpoint_status": 200,
        "list_endpoint_status": 200,
        "empty_list": False,
        "version_endpoint_status": 200,
        "screenshot_size": 200_000,
        "connected_clients": 0,
        "received_commands": [],
    }

    async def process_request(connection, request):
        port = connection.local_address[1]

        if request.path.startswith("/json/new"):
            if server_control["new_endpoint_status"] != 200:
                return Response(server_control["new_endpoint_status"], "Error", Headers([]), b"Error")
            body = json.dumps({
                "id": "target-page-stress",
                "type": "page",
                "title": "Aura Stress Target",
                "url": "about:blank",
                "webSocketDebuggerUrl": f"ws://127.0.0.1:{port}/devtools/page/stress",
            }).encode()
            headers = Headers([
                ("Content-Type", "application/json"),
                ("Content-Length", str(len(body))),
            ])
            return Response(200, "OK", headers, body)

        elif request.path.startswith("/json/list"):
            if server_control["list_endpoint_status"] != 200:
                return Response(server_control["list_endpoint_status"], "Error", Headers([]), b"Error")
            if server_control["empty_list"]:
                body = b"[]"
            else:
                body = json.dumps([{
                    "id": "target-page-stress",
                    "type": "page",
                    "title": "Aura Stress Target",
                    "url": "about:blank",
                    "webSocketDebuggerUrl": f"ws://127.0.0.1:{port}/devtools/page/stress",
                }]).encode()
            headers = Headers([
                ("Content-Type", "application/json"),
                ("Content-Length", str(len(body))),
            ])
            return Response(200, "OK", headers, body)

        elif request.path.startswith("/json/version"):
            if server_control["version_endpoint_status"] != 200:
                return Response(server_control["version_endpoint_status"], "Error", Headers([]), b"Error")
            body = json.dumps({
                "Browser": "HeadlessChrome/120.0.0",
                "Protocol-Version": "1.3",
                "webSocketDebuggerUrl": f"ws://127.0.0.1:{port}/devtools/browser/root",
            }).encode()
            headers = Headers([
                ("Content-Type", "application/json"),
                ("Content-Length", str(len(body))),
            ])
            return Response(200, "OK", headers, body)

        return None

    async def ws_handler(websocket):
        server_control["connected_clients"] += 1
        try:
            async for raw_msg in websocket:
                if server_control["force_disconnect"]:
                    await websocket.close(1000, "Forced disconnection")
                    break

                if server_control["delay_seconds"] > 0:
                    await asyncio.sleep(server_control["delay_seconds"])

                msg = json.loads(raw_msg)
                cmd_id = msg.get("id")
                method = msg.get("method")
                params = msg.get("params", {})
                server_control["received_commands"].append({"id": cmd_id, "method": method})

                if server_control["inject_unsolicited_events"]:
                    event_msg = {
                        "method": "Page.frameNavigated",
                        "params": {"frame": {"id": "frame-1", "url": "about:blank"}},
                    }
                    await websocket.send(json.dumps(event_msg))
                    event_msg2 = {
                        "method": "Network.requestWillBeSent",
                        "params": {"requestId": "req-1", "request": {"url": "https://aura.local/res"}},
                    }
                    await websocket.send(json.dumps(event_msg2))

                if method == "Page.navigate":
                    resp = {
                        "id": cmd_id,
                        "result": {"frameId": "frame-uuid-stress", "loaderId": "loader-uuid-stress"},
                    }
                    await websocket.send(json.dumps(resp))

                elif method == "Runtime.evaluate":
                    expr = params.get("expression", "")
                    if ">>>" in expr or "invalid_selector" in expr or "SyntaxError" in expr:
                        resp = {
                            "id": cmd_id,
                            "result": {
                                "exceptionDetails": {
                                    "text": "DOMException: Failed to execute 'querySelector': '>>>' is not a valid selector.",
                                    "exception": {
                                        "description": "DOMException: Failed to execute 'querySelector': '>>>' is not a valid selector."
                                    },
                                }
                            },
                        }
                    elif "nonexistent" in expr:
                        resp = {
                            "id": cmd_id,
                            "result": {
                                "exceptionDetails": {
                                    "text": "Error: Element not found: #nonexistent",
                                    "exception": {"description": "Error: Element not found: #nonexistent"},
                                }
                            },
                        }
                    elif "throw_error" in expr:
                        resp = {
                            "id": cmd_id,
                            "result": {
                                "exceptionDetails": {
                                    "text": "CustomException: Evaluation crashed",
                                    "exception": {"description": "CustomException: Evaluation crashed"},
                                }
                            },
                        }
                    elif "complex_object" in expr:
                        resp = {
                            "id": cmd_id,
                            "result": {
                                "result": {
                                    "type": "object",
                                    "value": {"status": "success", "count": 42, "items": ["a", "b"]},
                                }
                            },
                        }
                    elif "null_value" in expr:
                        resp = {
                            "id": cmd_id,
                            "result": {"result": {"type": "object", "subtype": "null", "value": None}},
                        }
                    elif "undefined_value" in expr:
                        resp = {
                            "id": cmd_id,
                            "result": {"result": {"type": "undefined"}},
                        }
                    elif "click()" in expr:
                        resp = {
                            "id": cmd_id,
                            "result": {"result": {"type": "boolean", "value": True}},
                        }
                    else:
                        resp = {
                            "id": cmd_id,
                            "result": {"result": {"type": "string", "value": f"eval_echo:{expr}"}},
                        }
                    await websocket.send(json.dumps(resp))

                elif method == "Page.captureScreenshot":
                    sz = server_control["screenshot_size"]
                    large_bytes = PNG_MAGIC_BYTES + (b"A" * max(0, sz - len(PNG_MAGIC_BYTES)))
                    b64_str = base64.b64encode(large_bytes).decode("utf-8")
                    resp = {"id": cmd_id, "result": {"data": b64_str}}
                    await websocket.send(json.dumps(resp))

                else:
                    resp = {"id": cmd_id, "result": {}}
                    await websocket.send(json.dumps(resp))
        finally:
            server_control["connected_clients"] -= 1

    server = await serve(ws_handler, "127.0.0.1", 0, process_request=process_request, max_size=32*1024*1024)
    port = server.sockets[0].getsockname()[1]
    cdp_url = f"http://127.0.0.1:{port}"

    yield {"cdp_url": cdp_url, "port": port, "control": server_control}

    server.close()
    await server.wait_closed()


# ------------------ BASELINE TESTS ------------------

@pytest.mark.asyncio
async def test_baseline_async_cdp_client_navigation(baseline_cdp_emulator):
    client = AsyncCdpClient(baseline_cdp_emulator["cdp_url"])
    res = await client.navigate("https://aura.local/welcome")
    assert "frameId" in res
    assert res["frameId"] == "frame-uuid-123"
    assert "https://aura.local/welcome" in baseline_cdp_emulator["state"]["navigated_urls"]
    await client.close()


@pytest.mark.asyncio
async def test_baseline_browser_tool_navigate_and_evaluate(baseline_cdp_emulator):
    tool = BrowserTool(cdp_url=baseline_cdp_emulator["cdp_url"])
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
async def test_baseline_browser_tool_click_success_and_failure(baseline_cdp_emulator):
    tool = BrowserTool(cdp_url=baseline_cdp_emulator["cdp_url"])
    click_res = await tool.click("#submit-button")
    assert click_res["status"] == "ok"
    assert click_res["selector"] == "#submit-button"

    with pytest.raises(RuntimeError) as exc_info:
        await tool.click("#nonexistent")
    assert "Element not found" in str(exc_info.value)

    await tool.close()


@pytest.mark.asyncio
async def test_baseline_browser_tool_screenshot(baseline_cdp_emulator):
    tool = BrowserTool(cdp_url=baseline_cdp_emulator["cdp_url"])
    png_bytes = await tool.screenshot(format="png")
    assert isinstance(png_bytes, bytes)
    assert png_bytes.startswith(PNG_MAGIC_BYTES)
    assert b"IMAGE_STREAM_TEST_PAYLOAD" in png_bytes
    await tool.close()


@pytest.mark.asyncio
async def test_baseline_browser_tool_syntax_error_evaluation(baseline_cdp_emulator):
    tool = BrowserTool(cdp_url=baseline_cdp_emulator["cdp_url"])
    with pytest.raises(RuntimeError) as exc_info:
        await tool.evaluate("SYNTAX_ERROR(;;)")
    assert "SyntaxError" in str(exc_info.value)
    await tool.close()


@pytest.mark.asyncio
async def test_baseline_zero_trust_unreachable_endpoint():
    tool = BrowserTool(cdp_url="http://127.0.0.1:49999")
    with pytest.raises(Exception):
        await tool.navigate("https://aura.local")


# ------------------ REMEDIATION TESTS ------------------

@pytest.mark.asyncio
async def test_remediated_screenshot_under_and_over_1mb(stress_cdp_server):
    """Verifies that both small (<1MB) and large (>1MB, e.g. 3.5MB) screenshots succeed."""
    tool = BrowserTool(cdp_url=stress_cdp_server["cdp_url"])

    # Under 1MB
    stress_cdp_server["control"]["screenshot_size"] = 500_000
    png1 = await tool.screenshot(format="png")
    assert len(png1) == 500_000

    # Over 1MB (3.5MB)
    stress_cdp_server["control"]["screenshot_size"] = 3_500_000
    png2 = await tool.screenshot(format="png")
    assert len(png2) == 3_500_000
    assert png2.startswith(PNG_MAGIC_BYTES)

    await tool.close()


@pytest.mark.asyncio
async def test_remediated_cold_client_concurrent_connect_synchronized(stress_cdp_server):
    """Verifies that concurrent calls on a cold client are synchronized and complete with 0 timeouts."""
    tool = BrowserTool(cdp_url=stress_cdp_server["cdp_url"])

    async def call_worker(idx: int):
        return await tool.evaluate(f"cold_call_{idx}")

    tasks = [call_worker(i) for i in range(10)]
    results = await asyncio.wait_for(asyncio.gather(*tasks), timeout=5.0)

    for i in range(10):
        assert f"eval_echo:cold_call_{i}" in results

    await tool.close()


@pytest.mark.asyncio
async def test_remediated_server_disconnect_auto_reconnects(stress_cdp_server):
    """
    Verifies that when server disconnects mid-flight, self._ws is reset to None,
    enabling subsequent commands to automatically reconnect and execute cleanly.
    """
    tool = BrowserTool(cdp_url=stress_cdp_server["cdp_url"])
    res = await tool.evaluate("before_disconnect")
    assert res == "eval_echo:before_disconnect"

    # Force server disconnect
    stress_cdp_server["control"]["force_disconnect"] = True
    with pytest.raises(websockets.exceptions.ConnectionClosed):
        await tool.evaluate("trigger_disconnect")

    # Verify that self._ws was cleared
    assert not tool._cdp_client.is_connected
    assert tool._cdp_client._ws is None

    # Server recovers
    stress_cdp_server["control"]["force_disconnect"] = False

    # The subsequent command must automatically reconnect and succeed!
    recovered = await tool.evaluate("after_disconnect_auto_recovered")
    assert recovered == "eval_echo:after_disconnect_auto_recovered"
    assert tool._cdp_client.is_connected

    await tool.close()


@pytest.mark.asyncio
async def test_remediated_lifecycle_close_and_reopen(stress_cdp_server):
    """Verifies clean close and re-connection lifecycle."""
    tool = BrowserTool(cdp_url=stress_cdp_server["cdp_url"])

    nav1 = await tool.navigate("https://aura.local/page1")
    assert nav1["status"] == "ok"

    await tool.close()
    assert tool._cdp_client._ws is None
    assert not tool._cdp_client.is_connected

    nav2 = await tool.navigate("https://aura.local/page2")
    assert nav2["status"] == "ok"
    assert tool._cdp_client.is_connected

    await tool.close()
