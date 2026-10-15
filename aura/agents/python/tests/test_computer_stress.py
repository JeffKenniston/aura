"""
Empirical adversarial stress test suite for AsyncComputerClient and ComputerTool.

Tests:
1. Server socket drop & auto-reconnect:
   - Sudden socket drop mid-flight during request.
   - Sudden socket drop while idle between commands.
   - Intermittent repeated server drops across a 30-command sequence.
   - Verification of zero zombie state, reader/writer nullification, and clean recovery.
2. Concurrent command execution under self._lock:
   - 50 concurrent tasks dispatching heterogeneous commands (click, move, drag, type, key, screenshot).
   - Verification of zero deadlocks, zero cross-talk, zero malformed JSON-RPC frames.
   - 100-command high-contention burst asserting 100% completion and lock safety.
3. close() lifecycle while operations are pending:
   - close() invoked concurrently while commands are queued / awaiting server response.
   - close() invoked concurrently from multiple tasks (concurrency idempotency).
   - clean termination without hang (bounded timeout assertions).
   - Subsequent command execution after close() cleanly reconnects without zombie state.
"""

from __future__ import annotations

import asyncio
import base64
import json
from typing import Any

import pytest

from aura_agents.tools.computer import (
    AsyncComputerClient,
    ComputerTool,
)
from tests.test_computer_tool import create_test_png


class StressMockDaemon:
    """Configurable mock JSON-RPC 2.0 daemon for empirical stress testing."""

    def __init__(self, sample_png: bytes | None = None, command_delay: float = 0.0):
        self.sample_png = sample_png or create_test_png(16, 16)
        self.command_delay = command_delay
        self.server: asyncio.Server | None = None
        self.host = "127.0.0.1"
        self.port: int = 0
        self.received_commands: list[dict[str, Any]] = []
        self.active_writers: list[asyncio.StreamWriter] = []
        self.drop_on_command_id: int | None = None
        self.drop_interval: int | None = None
        self._command_counter: int = 0
        self._lock = asyncio.Lock()

    async def start(self) -> None:
        self.server = await asyncio.start_server(self._handle_client, self.host, 0)
        self.port = self.server.sockets[0].getsockname()[1]

    async def _handle_client(
        self, reader: asyncio.StreamReader, writer: asyncio.StreamWriter
    ) -> None:
        async with self._lock:
            self.active_writers.append(writer)

        try:
            while True:
                line = await reader.readline()
                if not line:
                    break

                async with self._lock:
                    self._command_counter += 1
                    current_count = self._command_counter

                req = json.loads(line.decode("utf-8").strip())
                async with self._lock:
                    self.received_commands.append(req)

                req_id = req.get("id")
                method = req.get("method")
                params = req.get("params", {})

                # Check if drop triggered on specific command count or interval
                if self.drop_on_command_id == req_id:
                    writer.close()
                    await writer.wait_closed()
                    break

                if self.drop_interval and (current_count % self.drop_interval == 0):
                    writer.close()
                    await writer.wait_closed()
                    break

                if self.command_delay > 0:
                    await asyncio.sleep(self.command_delay)

                # Generate response
                if method == "click":
                    result = {
                        "status": "ok",
                        "action": "click",
                        "x": params.get("x"),
                        "y": params.get("y"),
                        "button": params.get("button", 1),
                        "clicks": params.get("clicks", 1),
                    }
                elif method == "move":
                    result = {
                        "status": "ok",
                        "action": "move",
                        "x": params.get("x"),
                        "y": params.get("y"),
                    }
                elif method == "drag":
                    result = {
                        "status": "ok",
                        "action": "drag",
                        "start_x": params.get("start_x"),
                        "start_y": params.get("start_y"),
                        "end_x": params.get("end_x"),
                        "end_y": params.get("end_y"),
                    }
                elif method == "type":
                    result = {
                        "status": "ok",
                        "action": "type",
                        "text": params.get("text"),
                    }
                elif method == "key":
                    result = {
                        "status": "ok",
                        "action": "key",
                        "key": params.get("key"),
                        "action_type": params.get("action", "press"),
                    }
                elif method == "screenshot":
                    b64 = base64.b64encode(self.sample_png).decode("ascii")
                    result = {
                        "status": "ok",
                        "format": "png",
                        "png_base64": b64,
                        "data": b64,
                    }
                else:
                    result = {"status": "ok", "method": method}

                resp = {"jsonrpc": "2.0", "id": req_id, "result": result}
                writer.write((json.dumps(resp) + "\n").encode("utf-8"))
                await writer.drain()
        except (ConnectionResetError, BrokenPipeError):
            pass
        finally:
            async with self._lock:
                if writer in self.active_writers:
                    self.active_writers.remove(writer)
            try:
                writer.close()
                await writer.wait_closed()
            except Exception:
                pass

    async def drop_all_connections(self) -> None:
        """Forcibly closes all active client connections from server side."""
        async with self._lock:
            writers = list(self.active_writers)
        for w in writers:
            try:
                w.close()
                await w.wait_closed()
            except Exception:
                pass

    async def stop(self) -> None:
        await self.drop_all_connections()
        if self.server is not None:
            self.server.close()
            await self.server.wait_closed()


# ============================================================================
# 1. Server Socket Drop & Reconnect Tests
# ============================================================================


@pytest.mark.asyncio
async def test_sudden_socket_drop_mid_session_and_clean_reconnect():
    """
    Simulates sudden socket drop mid-session while idle, issues next command,
    and asserts clean re-connection without zombie state.
    """
    daemon = StressMockDaemon()
    await daemon.start()

    client = AsyncComputerClient(host=daemon.host, port=daemon.port, timeout=2.0)

    try:
        # 1. Initial command succeeds
        res1 = await client.send_command("move", {"x": 100, "y": 200})
        assert res1["action"] == "move"
        assert res1["x"] == 100
        assert client.is_connected is True
        assert client._writer is not None
        assert client._reader is not None

        # 2. Server drops socket mid-session while connection is idle
        await daemon.drop_all_connections()
        # Small yield to let OS deliver TCP FIN
        await asyncio.sleep(0.05)

        # 3. Next command issued: client detects socket EOF/drop, cleans up zombie handles,
        #    and establishes a fresh connection.
        #    Note: if write succeeds but readline encounters EOF, ConnectionResetError is raised
        #    and handles are purged to None.
        try:
            res2 = await client.send_command("move", {"x": 300, "y": 400})
            assert res2["x"] == 300
        except ConnectionResetError:
            # First attempt cleanly caught drop and purged state
            assert client.is_connected is False
            assert client._writer is None
            assert client._reader is None
            # Next attempt re-connects cleanly
            res2 = await client.send_command("move", {"x": 300, "y": 400})
            assert res2["x"] == 300

        # Assert connection is healthy and active without zombie state
        assert client.is_connected is True
        assert client._writer is not None
        assert client._reader is not None

        # 4. Subsequent commands execute normally
        res3 = await client.send_command("click", {"x": 300, "y": 400, "button": 1})
        assert res3["action"] == "click"
        assert client.is_connected is True
    finally:
        await client.close()
        await daemon.stop()


@pytest.mark.asyncio
async def test_sudden_socket_drop_mid_flight_and_tool_recovery():
    """
    Simulates server drop mid-flight during request processing, asserts state purge,
    and asserts next tool invocation reconnects cleanly.
    """
    daemon = StressMockDaemon()
    daemon.drop_on_command_id = 2  # drop specifically during command id 2
    await daemon.start()

    tool = ComputerTool(host=daemon.host, port=daemon.port)

    try:
        # Command 1 succeeds
        c1 = await tool.move(50, 50)
        assert c1["status"] == "ok"
        client = await tool._get_client()
        assert client.is_connected is True

        # Command 2 gets dropped mid-flight
        with pytest.raises(ConnectionResetError):
            await tool.move(100, 100)

        # Assert no zombie state: writer and reader reset
        assert client.is_connected is False
        assert client._writer is None
        assert client._reader is None

        # Command 3 automatically re-connects and succeeds
        c3 = await tool.move(150, 150)
        assert c3["status"] == "ok"
        assert c3["x"] == 150
        assert client.is_connected is True
    finally:
        await tool.close()
        await daemon.stop()


@pytest.mark.asyncio
async def test_repeated_intermittent_socket_drops_resilience():
    """
    Simulates repeated socket drops across 25 sequential commands.
    Asserts client never hangs, does not deadlock, and restores connectivity every time.
    """
    daemon = StressMockDaemon()
    daemon.drop_interval = 4  # drops every 4th command
    await daemon.start()

    client = AsyncComputerClient(host=daemon.host, port=daemon.port, timeout=2.0)
    successes = 0
    drops_handled = 0

    try:
        for i in range(1, 26):
            try:
                res = await client.send_command("move", {"x": i, "y": i})
                assert res["x"] == i
                successes += 1
            except ConnectionResetError:
                drops_handled += 1
                assert client.is_connected is False
                assert client._writer is None
                assert client._reader is None
                # Immediately retry the command
                res = await client.send_command("move", {"x": i, "y": i})
                assert res["x"] == i
                successes += 1

        assert drops_handled >= 4
        assert successes == 25
    finally:
        await client.close()
        await daemon.stop()


# ============================================================================
# 2. Concurrent Command Execution Under self._lock Tests
# ============================================================================


@pytest.mark.asyncio
async def test_concurrent_heterogeneous_commands_no_deadlock():
    """
    Dispatches 50 concurrent tasks with heterogeneous actions (click, move, drag, type, key, screenshot).
    Verifies that:
    1. Zero deadlocks occur under self._lock.
    2. Every task receives its correct matching response (no cross-talk).
    3. The daemon receives all 50 commands with valid framing.
    """
    daemon = StressMockDaemon(command_delay=0.002)
    await daemon.start()

    tool = ComputerTool(host=daemon.host, port=daemon.port)

    async def execute_task(task_idx: int) -> dict[str, Any]:
        mod = task_idx % 6
        if mod == 0:
            res = await tool.click(task_idx * 10, task_idx * 10, button=1, clicks=1)
            assert res["action"] == "click"
            assert res["x"] == task_idx * 10
            return res
        elif mod == 1:
            res = await tool.move(task_idx * 5, task_idx * 5)
            assert res["action"] == "move"
            assert res["x"] == task_idx * 5
            return res
        elif mod == 2:
            res = await tool.drag(task_idx, task_idx, task_idx + 10, task_idx + 10)
            assert res["action"] == "drag"
            assert res["start_x"] == task_idx
            return res
        elif mod == 3:
            text = f"cmd_{task_idx}"
            res = await tool.type(text)
            assert res["action"] == "type"
            assert res["text"] == text
            return res
        elif mod == 4:
            res = await tool.key("Return", action="press")
            assert res["action"] == "key"
            assert res["key"] == "Return"
            return res
        else:
            png = await tool.screenshot()
            assert png.startswith(b"\x89PNG")
            return {"status": "ok", "action": "screenshot"}

    try:
        tasks = [execute_task(i) for i in range(50)]
        results = await asyncio.wait_for(asyncio.gather(*tasks), timeout=10.0)

        assert len(results) == 50
        assert len(daemon.received_commands) == 50

        # Assert no ID duplication in received commands
        command_ids = [cmd["id"] for cmd in daemon.received_commands]
        assert len(set(command_ids)) == 50
    finally:
        await tool.close()
        await daemon.stop()


@pytest.mark.asyncio
async def test_high_contention_burst_concurrency():
    """
    Dispatches 100 concurrent tasks rapidly executing move operations.
    Asserts strict lock serialization without deadlock or dropped commands.
    """
    daemon = StressMockDaemon()
    await daemon.start()

    client = AsyncComputerClient(host=daemon.host, port=daemon.port, timeout=5.0)

    async def single_move(idx: int) -> int:
        resp = await client.send_command("move", {"x": idx, "y": idx * 2})
        return resp["x"]

    try:
        tasks = [single_move(i) for i in range(100)]
        results = await asyncio.wait_for(asyncio.gather(*tasks), timeout=10.0)

        assert len(results) == 100
        assert sorted(results) == list(range(100))
        assert len(daemon.received_commands) == 100
    finally:
        await client.close()
        await daemon.stop()


# ============================================================================
# 3. close() Lifecycle While Operations Pending Tests
# ============================================================================


@pytest.mark.asyncio
async def test_close_called_while_operations_pending_clean_termination():
    """
    Invokes close() while multiple operations are pending / in-flight.
    Asserts clean termination without hanging or deadlocking.
    """
    # Daemon introduces a 100ms delay per command
    daemon = StressMockDaemon(command_delay=0.1)
    await daemon.start()

    client = AsyncComputerClient(host=daemon.host, port=daemon.port, timeout=5.0)

    # Launch in-flight slow command
    slow_task = asyncio.create_task(client.send_command("move", {"x": 999, "y": 999}))

    # Wait 20ms so slow_task acquires the lock and enters readline()
    await asyncio.sleep(0.02)

    # Launch queued commands behind the slow command
    queued_task1 = asyncio.create_task(client.send_command("move", {"x": 1, "y": 1}))
    queued_task2 = asyncio.create_task(client.send_command("move", {"x": 2, "y": 2}))

    # Concurrently call close()
    close_task = asyncio.create_task(client.close())

    # Assert close_task and all operations terminate cleanly within bounded timeout (no hang)
    done, _ = await asyncio.wait_for(
        asyncio.wait([slow_task, queued_task1, queued_task2, close_task]),
        timeout=3.0,
    )

    assert len(done) == 4
    # The slow task should have succeeded
    assert slow_task.result()["x"] == 999

    # The close task should have completed cleanly with no exception
    assert close_task.exception() is None

    # Connection state should be cleanly closed
    assert client.is_connected is False
    assert client._writer is None
    assert client._reader is None

    await daemon.stop()


@pytest.mark.asyncio
async def test_concurrent_idempotent_close_calls():
    """
    Asserts that multiple concurrent close() invocations terminate cleanly and idempotently.
    """
    daemon = StressMockDaemon()
    await daemon.start()

    client = AsyncComputerClient(host=daemon.host, port=daemon.port)
    await client.connect()
    assert client.is_connected is True

    # 10 concurrent calls to close()
    close_tasks = [client.close() for _ in range(10)]
    await asyncio.wait_for(asyncio.gather(*close_tasks), timeout=2.0)

    assert client.is_connected is False
    assert client._writer is None
    assert client._reader is None

    # Extra close on already closed client is safe no-op
    await client.close()
    assert client.is_connected is False

    await daemon.stop()


@pytest.mark.asyncio
async def test_computer_tool_close_and_subsequent_reconnect():
    """
    Asserts that closing ComputerTool cleanly tears down the underlying client,
    and a subsequent call creates a new client and reconnects seamlessly.
    """
    daemon = StressMockDaemon()
    await daemon.start()

    tool = ComputerTool(host=daemon.host, port=daemon.port)

    # 1. Operation succeeds
    r1 = await tool.move(10, 20)
    assert r1["x"] == 10

    # 2. Explicitly close tool
    await asyncio.wait_for(tool.close(), timeout=2.0)
    assert tool._daemon_client is None

    # 3. Subsequent operation cleanly initializes a new client and connects
    r2 = await tool.move(30, 40)
    assert r2["x"] == 30
    assert tool._daemon_client is not None
    assert tool._daemon_client.is_connected is True

    await tool.close()
    await daemon.stop()
