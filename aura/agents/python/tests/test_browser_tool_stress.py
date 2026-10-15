"""
Empirical adversarial and stress tests for BrowserTool and AsyncCdpClient (TOOL-BRW-003).

Validates:
1. Unsolicited CDP events (e.g. Page.frameNavigated, Network.requestWillBeSent) interleaving.
2. Network timeouts on delayed responses.
3. Out-of-order response disambiguation by message ID.
4. Clean close and explicit re-connection lifecycle.
5. Invalid selectors (syntax error, non-existent, special characters).
6. Heavy concurrent calls serialized safely when connected.
7. Cold-client concurrency safely synchronized under lock (no race conditions or dropped responses).
8. Screenshot payload under 1MB and over 1MB (handled safely by max_size=32MB limit).
9. JavaScript evaluation data types, exceptions, and promises.
10. Target discovery fallback chain (/json/new -> /json/list -> /json/version).
11. Auto-reconnection on subsequent command after server drops connection mid-session.
"""

from __future__ import annotations

import asyncio
import base64
import json

import pytest
import websockets
from websockets.asyncio.server import serve
from websockets.datastructures import Headers
from websockets.http11 import Response

from aura_agents.tools.browser import AsyncCdpClient, BrowserTool

PNG_MAGIC_BYTES = b"\x89PNG\r\n\x1a\n"


@pytest.fixture
async def stress_cdp_server():
    """
    Advanced CDP emulator server with configurable delays, event streams,
    and disconnect triggers for empirical stress testing.
    """
    server_control = {
        "delay_seconds": 0.0,
        "inject_unsolicited_events": True,
        "force_disconnect": False,
        "new_endpoint_status": 200,
        "list_endpoint_status": 200,
        "empty_list": False,
        "version_endpoint_status": 200,
        "screenshot_size": 200_000,  # 200 KB default
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

                # Inject unsolicited events if enabled
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
                    # Generate payload based on screenshot_size
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


@pytest.mark.asyncio
async def test_unsolicited_events_interleaving_and_deserialization(stress_cdp_server):
    """Verifies that unsolicited CDP events are safely ignored and do not corrupt response matching."""
    tool = BrowserTool(cdp_url=stress_cdp_server["cdp_url"])

    nav_res = await tool.navigate("https://aura.local/stress")
    assert nav_res["status"] == "ok"
    assert nav_res["result"]["frameId"] == "frame-uuid-stress"

    eval_res = await tool.evaluate("1 + 1")
    assert eval_res == "eval_echo:1 + 1"

    click_res = await tool.click("button.action")
    assert click_res["status"] == "ok"

    await tool.close()


@pytest.mark.asyncio
async def test_invalid_selectors_syntax_and_missing(stress_cdp_server):
    """Verifies error handling for syntax errors, missing elements, and escaped selectors."""
    tool = BrowserTool(cdp_url=stress_cdp_server["cdp_url"])

    # Syntax error selector
    with pytest.raises(RuntimeError) as exc_info:
        await tool.click(">>>")
    assert "Click failed" in str(exc_info.value)
    assert "DOMException" in str(exc_info.value)

    # Missing element
    with pytest.raises(RuntimeError) as exc_info:
        await tool.click("#nonexistent")
    assert "Click failed: Error: Element not found" in str(exc_info.value)

    # Selector with quotes and special characters
    click_res = await tool.click('button[data-name="my \\"button\\""]')
    assert click_res["status"] == "ok"

    await tool.close()


@pytest.mark.asyncio
async def test_complex_evaluations_and_exceptions(stress_cdp_server):
    """Verifies evaluation of complex objects, null, undefined, and JS exceptions."""
    tool = BrowserTool(cdp_url=stress_cdp_server["cdp_url"])

    obj = await tool.evaluate("complex_object")
    assert isinstance(obj, dict)
    assert obj["status"] == "success"
    assert obj["count"] == 42
    assert obj["items"] == ["a", "b"]

    null_val = await tool.evaluate("null_value")
    assert null_val is None

    undef_val = await tool.evaluate("undefined_value")
    assert undef_val is None

    with pytest.raises(RuntimeError) as exc_info:
        await tool.evaluate("throw_error")
    assert "Evaluation error: CustomException" in str(exc_info.value)

    await tool.close()


@pytest.mark.asyncio
async def test_screenshot_under_1mb_succeeds(stress_cdp_server):
    """Verifies standard screenshot streaming (<1MB) succeeds."""
    tool = BrowserTool(cdp_url=stress_cdp_server["cdp_url"])
    stress_cdp_server["control"]["screenshot_size"] = 250_000
    png_bytes = await tool.screenshot(format="png")
    assert isinstance(png_bytes, bytes)
    assert png_bytes.startswith(PNG_MAGIC_BYTES)
    assert len(png_bytes) == 250_000
    await tool.close()


@pytest.mark.asyncio
async def test_screenshot_over_1mb_succeeds(stress_cdp_server):
    """
    Verifies that with max_size=32MB configured on websockets.connect,
    screenshots exceeding 1MB (e.g. 1.5MB) stream successfully without ConnectionClosedError.
    """
    tool = BrowserTool(cdp_url=stress_cdp_server["cdp_url"])
    # 1.5 MB screenshot
    stress_cdp_server["control"]["screenshot_size"] = 1_500_000

    png_bytes = await tool.screenshot(format="png")
    assert isinstance(png_bytes, bytes)
    assert png_bytes.startswith(PNG_MAGIC_BYTES)
    assert len(png_bytes) == 1_500_000
    await tool.close()


@pytest.mark.asyncio
async def test_high_concurrency_serialization_when_connected(stress_cdp_server):
    """Verifies that concurrent calls are serialized safely by internal lock once connected."""
    tool = BrowserTool(cdp_url=stress_cdp_server["cdp_url"])
    # Warm up connection sequentially
    await tool.evaluate("init")

    async def call_worker(idx: int):
        return await tool.evaluate(f"worker_{idx}")

    tasks = [call_worker(i) for i in range(20)]
    results = await asyncio.gather(*tasks)

    for i in range(20):
        assert f"eval_echo:worker_{i}" in results

    await tool.close()


@pytest.mark.asyncio
async def test_cold_client_concurrent_connect_synchronized(stress_cdp_server):
    """
    Verifies that cold-client concurrent requests are synchronized under self._lock,
    preventing race conditions, duplicate connections, and dropped responses.
    """
    tool = BrowserTool(cdp_url=stress_cdp_server["cdp_url"])

    async def call_worker(idx: int):
        return await tool.evaluate(f"cold_{idx}")

    tasks = [call_worker(i) for i in range(5)]
    results = await asyncio.wait_for(asyncio.gather(*tasks), timeout=5.0)

    for i in range(5):
        assert f"eval_echo:cold_{i}" in results

    await tool.close()


@pytest.mark.asyncio
async def test_network_timeout_and_out_of_order_recovery(stress_cdp_server):
    """Verifies client timeout enforcement and subsequent recovery."""
    client = AsyncCdpClient(stress_cdp_server["cdp_url"])
    await client.connect()

    stress_cdp_server["control"]["delay_seconds"] = 0.2

    # Call with timeout lower than server delay
    with pytest.raises(asyncio.TimeoutError):
        await client.send_command("Page.navigate", {"url": "https://aura.local/timeout"}, timeout=0.05)

    # Reset delay and verify subsequent command succeeds despite previous delayed response
    stress_cdp_server["control"]["delay_seconds"] = 0.0
    res = await client.send_command("Runtime.evaluate", {"expression": "recovered"})
    assert res.get("result", {}).get("value") == "eval_echo:recovered"

    await client.close()


@pytest.mark.asyncio
async def test_target_discovery_fallbacks(stress_cdp_server):
    """Verifies fallback chain: /json/new (fails) -> /json/list (fails) -> /json/version (succeeds)."""
    stress_cdp_server["control"]["new_endpoint_status"] = 404
    stress_cdp_server["control"]["empty_list"] = True

    tool = BrowserTool(cdp_url=stress_cdp_server["cdp_url"])
    nav = await tool.navigate("https://aura.local/fallback")
    assert nav["status"] == "ok"
    assert nav["result"]["frameId"] == "frame-uuid-stress"
    await tool.close()


@pytest.mark.asyncio
async def test_close_and_reconnection_lifecycle(stress_cdp_server):
    """Verifies clean close and seamless re-connection upon next request."""
    tool = BrowserTool(cdp_url=stress_cdp_server["cdp_url"])

    nav1 = await tool.navigate("https://aura.local/page1")
    assert nav1["status"] == "ok"

    await tool.close()
    assert tool._cdp_client._ws is None

    # Re-use tool after close
    nav2 = await tool.navigate("https://aura.local/page2")
    assert nav2["status"] == "ok"

    await tool.close()


@pytest.mark.asyncio
async def test_server_disconnect_auto_reconnects(stress_cdp_server):
    """
    Verifies that server disconnect mid-session resets self._ws = None,
    enabling subsequent commands to automatically reconnect and succeed
    without requiring manual tool.close().
    """
    tool = BrowserTool(cdp_url=stress_cdp_server["cdp_url"])
    res = await tool.evaluate("before_disconnect")
    assert res == "eval_echo:before_disconnect"

    # Force server disconnect
    stress_cdp_server["control"]["force_disconnect"] = True
    with pytest.raises(websockets.exceptions.ConnectionClosed):
        await tool.evaluate("trigger_disconnect")

    # Server is back up
    stress_cdp_server["control"]["force_disconnect"] = False

    # Attempting to send again automatically reconnects because self._ws was cleared
    recovered = await tool.evaluate("after_disconnect_auto_reconnect")
    assert recovered == "eval_echo:after_disconnect_auto_reconnect"

    await tool.close()
