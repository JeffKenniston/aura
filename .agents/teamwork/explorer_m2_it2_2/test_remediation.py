"""
Pytest unit tests for AsyncComputerClient connection resilience.
"""

import asyncio
import json
from typing import Any
import pytest

from aura_sdk.tools.computer import ComputerTool


class ResilientAsyncComputerClient:
    """
    Resilient Asynchronous JSON-RPC 2.0 client for Firecracker microVM computer daemon.
    """

    def __init__(self, host: str = "127.0.0.1", port: int = 5254, timeout: float = 10.0):
        self.host = host
        self.port = port
        self.timeout = timeout
        self._reader: asyncio.StreamReader | None = None
        self._writer: asyncio.StreamWriter | None = None
        self._msg_id: int = 0
        self._lock = asyncio.Lock()

    @property
    def is_connected(self) -> bool:
        if self._writer is None or self._reader is None:
            return False
        if self._writer.is_closing() or self._reader.at_eof():
            return False
        return True

    async def connect(self) -> None:
        async with self._lock:
            await self._connect_locked()

    async def _close_locked(self) -> None:
        """Closes stream reader and writer, resetting connection state while holding lock."""
        if self._writer is not None:
            try:
                self._writer.close()
                await self._writer.wait_closed()
            except Exception:
                pass
            self._writer = None
        self._reader = None

    async def _connect_locked(self) -> None:
        if self.is_connected:
            return
        await self._close_locked()
        self._reader, self._writer = await asyncio.wait_for(
            asyncio.open_connection(self.host, self.port), timeout=self.timeout
        )

    async def send_command(self, method: str, params: dict[str, Any] | None = None) -> dict[str, Any]:
        async with self._lock:
            try:
                if not self.is_connected:
                    await self._connect_locked()
                assert self._reader is not None
                assert self._writer is not None

                self._msg_id += 1
                cmd_id = self._msg_id
                payload = {
                    "jsonrpc": "2.0",
                    "id": cmd_id,
                    "method": method,
                    "params": params or {},
                }
                raw_line = json.dumps(payload) + "\n"
                self._writer.write(raw_line.encode("utf-8"))
                await self._writer.drain()

                line_bytes = await asyncio.wait_for(
                    self._reader.readline(), timeout=self.timeout
                )
                if not line_bytes:
                    raise ConnectionResetError("Connection closed by computer daemon")

                resp = json.loads(line_bytes.decode("utf-8").strip())
            except Exception:
                await self._close_locked()
                raise

            if "error" in resp and resp["error"] is not None:
                err = resp["error"]
                msg = err.get("message", "Unknown error")
                code = err.get("code", -32603)
                raise RuntimeError(f"Computer daemon error ({code}): {msg}")

            return resp.get("result", {})

    async def close(self) -> None:
        async with self._lock:
            await self._close_locked()


@pytest.mark.asyncio
async def test_resilient_client_drop_and_reconnect():
    cmd_count = 0

    async def handle_client(reader: asyncio.StreamReader, writer: asyncio.StreamWriter):
        nonlocal cmd_count
        while True:
            line = await reader.readline()
            if not line:
                break
            cmd_count += 1
            if cmd_count == 1:
                writer.write(b'{"jsonrpc": "2.0", "id": 1, "result": {"status": "ok_1"}}\n')
                await writer.drain()
            elif cmd_count == 2:
                # Abruptly sever connection mid-flight on 2nd command
                writer.close()
                await writer.wait_closed()
                break
            else:
                # Subsequent command on new connection
                req = json.loads(line.decode("utf-8").strip())
                resp = {"jsonrpc": "2.0", "id": req["id"], "result": {"status": "ok_reconnected"}}
                writer.write(json.dumps(resp).encode("utf-8") + b"\n")
                await writer.drain()

    server = await asyncio.start_server(handle_client, "127.0.0.1", 0)
    port = server.sockets[0].getsockname()[1]
    client = ResilientAsyncComputerClient(host="127.0.0.1", port=port, timeout=2.0)

    # 1. Command 1 succeeds
    r1 = await client.send_command("move", {"x": 10, "y": 10})
    assert r1 == {"status": "ok_1"}
    assert client.is_connected is True

    # 2. Command 2 fails mid-flight when server abruptly drops
    with pytest.raises(ConnectionResetError, match="Connection closed by computer daemon"):
        await client.send_command("move", {"x": 20, "y": 20})

    # Assert client handles are reset to None and is_connected is False
    assert client.is_connected is False
    assert client._writer is None
    assert client._reader is None

    # 3. Command 3 automatically reconnects and succeeds!
    r3 = await client.send_command("move", {"x": 30, "y": 30})
    assert r3 == {"status": "ok_reconnected"}
    assert client.is_connected is True

    await client.close()
    server.close()
    await server.wait_closed()


@pytest.mark.asyncio
async def test_resilient_client_application_error_preserves_connection():
    async def handle_client(reader: asyncio.StreamReader, writer: asyncio.StreamWriter):
        while True:
            line = await reader.readline()
            if not line:
                break
            req = json.loads(line.decode("utf-8").strip())
            if req.get("method") == "bad_method":
                resp = {"jsonrpc": "2.0", "id": req["id"], "error": {"code": -32601, "message": "Method not found"}}
            else:
                resp = {"jsonrpc": "2.0", "id": req["id"], "result": {"status": "ok_good"}}
            writer.write(json.dumps(resp).encode("utf-8") + b"\n")
            await writer.drain()

    server = await asyncio.start_server(handle_client, "127.0.0.1", 0)
    port = server.sockets[0].getsockname()[1]
    client = ResilientAsyncComputerClient(host="127.0.0.1", port=port, timeout=2.0)

    # 1. Valid command 1
    r1 = await client.send_command("good_method")
    assert r1 == {"status": "ok_good"}
    assert client.is_connected is True

    # 2. Application error: does not close connection
    with pytest.raises(RuntimeError, match="Method not found"):
        await client.send_command("bad_method")

    assert client.is_connected is True
    assert client._writer is not None
    assert client._reader is not None

    # 3. Following command reuses existing connection without reconnection overhead
    r2 = await client.send_command("good_method")
    assert r2 == {"status": "ok_good"}
    assert client.is_connected is True

    await client.close()
    server.close()
    await server.wait_closed()


@pytest.mark.asyncio
async def test_resilient_client_read_timeout_cleanup():
    async def handle_client(reader: asyncio.StreamReader, writer: asyncio.StreamWriter):
        line = await reader.readline()
        await asyncio.sleep(5.0)

    server = await asyncio.start_server(handle_client, "127.0.0.1", 0)
    port = server.sockets[0].getsockname()[1]
    client = ResilientAsyncComputerClient(host="127.0.0.1", port=port, timeout=0.1)

    with pytest.raises(asyncio.TimeoutError):
        await client.send_command("hang_method")

    assert client.is_connected is False
    assert client._writer is None
    assert client._reader is None

    await client.close()
    server.close()
    await server.wait_closed()


@pytest.mark.asyncio
async def test_resilient_client_concurrency():
    async def handle_client(reader: asyncio.StreamReader, writer: asyncio.StreamWriter):
        while True:
            line = await reader.readline()
            if not line:
                break
            req = json.loads(line.decode("utf-8").strip())
            resp = {"jsonrpc": "2.0", "id": req["id"], "result": {"status": "ok", "echo_id": req["id"]}}
            writer.write(json.dumps(resp).encode("utf-8") + b"\n")
            await writer.drain()

    server = await asyncio.start_server(handle_client, "127.0.0.1", 0)
    port = server.sockets[0].getsockname()[1]
    client = ResilientAsyncComputerClient(host="127.0.0.1", port=port, timeout=2.0)

    tasks = [client.send_command("move", {"x": i, "y": i}) for i in range(10)]
    results = await asyncio.gather(*tasks)
    assert len(results) == 10
    assert sorted([r["echo_id"] for r in results]) == list(range(1, 11))

    await client.close()
    server.close()
    await server.wait_closed()


@pytest.mark.asyncio
async def test_computer_tool_integration_with_resilient_client():
    cmd_count = 0

    async def handle_client(reader: asyncio.StreamReader, writer: asyncio.StreamWriter):
        nonlocal cmd_count
        while True:
            line = await reader.readline()
            if not line:
                break
            cmd_count += 1
            req = json.loads(line.decode("utf-8").strip())
            if cmd_count == 2:
                # Drop on 2nd command
                writer.close()
                await writer.wait_closed()
                break
            resp = {"jsonrpc": "2.0", "id": req["id"], "result": {"status": "ok", "action": req["method"], "x": 100, "y": 200}}
            writer.write(json.dumps(resp).encode("utf-8") + b"\n")
            await writer.drain()

    server = await asyncio.start_server(handle_client, "127.0.0.1", 0)
    port = server.sockets[0].getsockname()[1]
    client = ResilientAsyncComputerClient(host="127.0.0.1", port=port, timeout=2.0)
    tool = ComputerTool(daemon_client=client)

    # 1. Action 1 succeeds
    res1 = await tool.move(10, 20)
    assert res1["status"] == "ok"

    # 2. Action 2 dropped
    with pytest.raises(ConnectionResetError):
        await tool.move(30, 40)

    # 3. Action 3 auto-reconnects and succeeds
    res3 = await tool.move(50, 60)
    assert res3["status"] == "ok"

    await tool.close()
    server.close()
    await server.wait_closed()
