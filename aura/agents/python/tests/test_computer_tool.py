"""
Unit and integration tests for Aura ComputerTool and PNG scanline visual redaction.
"""

from __future__ import annotations

import asyncio
import base64
import json
import struct
import zlib
from pathlib import Path
from typing import Any

import pytest

from aura_agents.tools.computer import (
    AsyncComputerClient,
    ComputerTool,
    _make_chunk,
    mask_image_bytes,
)


def create_test_png(
    width: int = 10,
    height: int = 10,
    color_type: int = 6,  # 6=RGBA, 2=RGB
    fill_color: tuple[int, ...] = (255, 255, 255, 255),
    filter_type: int = 0,
) -> bytes:
    """Helper creating a valid uncompressed/filtered PNG image in pure Python."""
    bpp = 4 if color_type == 6 else 3
    ihdr_data = struct.pack(">IIBBBBB", width, height, 8, color_type, 0, 0, 0)

    raw_scanlines = bytearray()
    for _ in range(height):
        raw_scanlines.append(filter_type)
        row = bytearray()
        for _ in range(width):
            row.extend(fill_color[:bpp])
        raw_scanlines.extend(row)

    compressed_idat = zlib.compress(bytes(raw_scanlines))

    png = bytearray(b"\x89PNG\r\n\x1a\n")
    png.extend(_make_chunk(b"IHDR", ihdr_data))
    png.extend(_make_chunk(b"IDAT", compressed_idat))
    png.extend(_make_chunk(b"IEND", b""))
    return bytes(png)


def decode_png_pixels(png_bytes: bytes) -> tuple[int, int, int, list[bytearray]]:
    """Helper decoding reconstructed pixel rows from PNG bytes for assertions."""
    idx = 8
    ihdr_data = None
    idat_parts = []
    while idx < len(png_bytes):
        chunk_len, chunk_type = struct.unpack(">I4s", png_bytes[idx : idx + 8])
        idx += 8
        chunk_data = png_bytes[idx : idx + chunk_len]
        idx += chunk_len + 4  # skip data + crc
        if chunk_type == b"IHDR":
            ihdr_data = chunk_data
        elif chunk_type == b"IDAT":
            idat_parts.append(chunk_data)

    assert ihdr_data is not None
    width, height, _, color_type, _, _, _ = struct.unpack(">IIBBBBB", ihdr_data)
    bpp = 4 if color_type == 6 else 3
    stride = width * bpp
    decompressed = zlib.decompress(b"".join(idat_parts))

    rows = []
    pos = 0
    for _ in range(height):
        # Filter byte
        pos += 1
        row = bytearray(decompressed[pos : pos + stride])
        pos += stride
        rows.append(row)
    return width, height, bpp, rows


class MockComputerDaemon:
    """Mock in-process JSON-RPC 2.0 daemon for testing ComputerTool."""

    def __init__(self, sample_png: bytes | None = None):
        self.sample_png = sample_png or create_test_png(16, 16)
        self.server: asyncio.Server | None = None
        self.host = "127.0.0.1"
        self.port: int = 0
        self.received_commands: list[dict[str, Any]] = []

    async def start(self) -> None:
        self.server = await asyncio.start_server(self._handle_client, self.host, 0)
        self.port = self.server.sockets[0].getsockname()[1]

    async def _handle_client(
        self, reader: asyncio.StreamReader, writer: asyncio.StreamWriter
    ) -> None:
        while True:
            line = await reader.readline()
            if not line:
                break
            req = json.loads(line.decode("utf-8").strip())
            self.received_commands.append(req)

            req_id = req.get("id")
            method = req.get("method")
            params = req.get("params", {})

            if method == "click":
                result = {
                    "status": "ok",
                    "action": "click",
                    "x": params.get("x"),
                    "y": params.get("y"),
                    "button": params.get("button", 1),
                    "clicks": params.get("clicks", 1),
                }
                resp = {"jsonrpc": "2.0", "id": req_id, "result": result}
            elif method == "move":
                result = {
                    "status": "ok",
                    "action": "move",
                    "x": params.get("x"),
                    "y": params.get("y"),
                }
                resp = {"jsonrpc": "2.0", "id": req_id, "result": result}
            elif method == "drag":
                result = {
                    "status": "ok",
                    "action": "drag",
                    "start_x": params.get("start_x", 0),
                    "start_y": params.get("start_y", 0),
                    "end_x": params.get("end_x", params.get("x", 0)),
                    "end_y": params.get("end_y", params.get("y", 0)),
                    "button": params.get("button", 1),
                }
                resp = {"jsonrpc": "2.0", "id": req_id, "result": result}
            elif method == "type":
                result = {
                    "status": "ok",
                    "action": "type",
                    "length": len(params.get("text", "")),
                }
                resp = {"jsonrpc": "2.0", "id": req_id, "result": result}
            elif method == "key":
                result = {
                    "status": "ok",
                    "action": "key",
                    "key": params.get("key"),
                    "action_type": params.get("action", "press"),
                }
                resp = {"jsonrpc": "2.0", "id": req_id, "result": result}
            elif method == "screenshot":
                b64 = base64.b64encode(self.sample_png).decode("ascii")
                result = {
                    "status": "ok",
                    "format": "png",
                    "png_base64": b64,
                    "data": b64,
                }
                resp = {"jsonrpc": "2.0", "id": req_id, "result": result}
            else:
                resp = {
                    "jsonrpc": "2.0",
                    "id": req_id,
                    "error": {"code": -32601, "message": "Method not found"},
                }

            writer.write((json.dumps(resp) + "\n").encode("utf-8"))
            await writer.drain()

        writer.close()
        await writer.wait_closed()

    async def stop(self) -> None:
        if self.server is not None:
            self.server.close()
            await self.server.wait_closed()


# ============================================================================
# Pure Python PNG Scanline Visual Redaction Tests
# ============================================================================


def test_mask_image_bytes_rgba_single_box():
    # 10x10 white image
    original = create_test_png(10, 10, color_type=6, fill_color=(255, 255, 255, 255))
    # Redact box (2, 2, 4, 4) -> x in 2..5, y in 2..5
    redacted = mask_image_bytes(original, [(2, 2, 4, 4)])

    assert redacted.startswith(b"\x89PNG\r\n\x1a\n")
    w, h, bpp, rows = decode_png_pixels(redacted)
    assert w == 10
    assert h == 10
    assert bpp == 4

    # Inside box: should be solid opaque black (0, 0, 0, 255)
    for y in range(2, 6):
        for x in range(2, 6):
            offset = x * 4
            pixel = list(rows[y][offset : offset + 4])
            assert pixel == [0, 0, 0, 255], f"Pixel at ({x}, {y}) is {pixel}"

    # Outside box: should remain white (255, 255, 255, 255)
    outside_coords = [(0, 0), (1, 1), (6, 6), (9, 9), (0, 9), (9, 0)]
    for x, y in outside_coords:
        offset = x * 4
        pixel = list(rows[y][offset : offset + 4])
        assert pixel == [255, 255, 255, 255], f"Pixel at ({x}, {y}) should be unmasked"


def test_mask_image_bytes_rgb_dict_box():
    original = create_test_png(12, 12, color_type=2, fill_color=(200, 200, 200))
    redacted = mask_image_bytes(
        original, [{"x": 3, "y": 3, "width": 5, "height": 4}]
    )

    w, h, bpp, rows = decode_png_pixels(redacted)
    assert w == 12
    assert h == 12
    assert bpp == 3

    # Inside masked region: (0, 0, 0)
    for y in range(3, 7):
        for x in range(3, 8):
            offset = x * 3
            assert list(rows[y][offset : offset + 3]) == [0, 0, 0]

    # Outside masked region: (200, 200, 200)
    assert list(rows[0][0:3]) == [200, 200, 200]
    assert list(rows[11][11 * 3 : 11 * 3 + 3]) == [200, 200, 200]


def test_mask_image_bytes_multiple_and_overlapping_boxes():
    original = create_test_png(20, 20, color_type=6, fill_color=(255, 255, 255, 255))
    boxes = [
        (0, 0, 5, 5),
        (3, 3, 5, 5),  # overlaps with first box
        {"x": 15, "y": 15, "w": 5, "h": 5},
    ]
    redacted = mask_image_bytes(original, boxes)
    _, _, _, rows = decode_png_pixels(redacted)

    # Box 1 & 2 overlap at (4, 4)
    assert list(rows[4][4 * 4 : 4 * 4 + 4]) == [0, 0, 0, 255]
    # Box 3 at (16, 16)
    assert list(rows[16][16 * 4 : 16 * 4 + 4]) == [0, 0, 0, 255]
    # Unmasked at (10, 10)
    assert list(rows[10][10 * 4 : 10 * 4 + 4]) == [255, 255, 255, 255]


def test_mask_image_bytes_empty_and_zero_boxes():
    original = create_test_png(8, 8, color_type=6)
    # Empty box list: noop
    res_empty = mask_image_bytes(original, [])
    assert res_empty.startswith(b"\x89PNG")
    _, _, _, rows = decode_png_pixels(res_empty)
    assert list(rows[0][0:4]) == [255, 255, 255, 255]

    # Zero width / height box: noop
    res_zero = mask_image_bytes(original, [(2, 2, 0, 0), (3, 3, -1, 5)])
    assert res_zero.startswith(b"\x89PNG")


def test_mask_image_bytes_clamped_boundaries():
    original = create_test_png(10, 10, color_type=6)
    # Box exceeding image boundary
    redacted = mask_image_bytes(original, [(-10, -10, 15, 15), (8, 8, 50, 50)])
    _, _, _, rows = decode_png_pixels(redacted)
    # Top-left corner (0, 0) is masked
    assert list(rows[0][0:4]) == [0, 0, 0, 255]
    # Bottom-right corner (9, 9) is masked
    assert list(rows[9][9 * 4 : 9 * 4 + 4]) == [0, 0, 0, 255]


def test_mask_image_bytes_filter_types_unfiltering():
    """Tests reconstruction across standard PNG filter algorithms (Sub, Up, Average, Paeth)."""
    width, height = 4, 4
    # Create raw scanlines with distinct values
    for filter_mode in [0, 1, 2, 3, 4]:
        png = create_test_png(
            width, height, color_type=6, fill_color=(100, 150, 200, 255), filter_type=filter_mode
        )
        redacted = mask_image_bytes(png, [(1, 1, 2, 2)])
        assert redacted.startswith(b"\x89PNG")
        _, _, _, rows = decode_png_pixels(redacted)
        assert list(rows[1][1 * 4 : 1 * 4 + 4]) == [0, 0, 0, 255]


def test_mask_image_bytes_invalid_header():
    with pytest.raises(ValueError, match="Invalid PNG signature"):
        mask_image_bytes(b"NOT_A_PNG_HEADER")


def test_mask_image_bytes_crc_integrity():
    original = create_test_png(10, 10)
    # Corrupt a byte in IDAT chunk
    corrupted = bytearray(original)
    corrupted[30] ^= 0xFF
    with pytest.raises(ValueError, match="CRC32 mismatch"):
        mask_image_bytes(bytes(corrupted), [(1, 1, 2, 2)])


# ============================================================================
# Async ComputerTool & Client Integration Tests
# ============================================================================


@pytest.mark.asyncio
async def test_computer_tool_all_input_methods():
    daemon = MockComputerDaemon()
    await daemon.start()

    tool = ComputerTool(host=daemon.host, port=daemon.port)

    # 1. Click
    res_click = await tool.click(200, 150, button=1, clicks=2)
    assert res_click["status"] == "ok"
    assert res_click["action"] == "click"
    assert res_click["x"] == 200
    assert res_click["y"] == 150
    assert res_click["clicks"] == 2

    # 2. Move
    res_move = await tool.move(400, 300)
    assert res_move["status"] == "ok"
    assert res_move["action"] == "move"
    assert res_move["x"] == 400
    assert res_move["y"] == 300

    # 3. Drag
    res_drag = await tool.drag(100, 100, 500, 300, button=1)
    assert res_drag["status"] == "ok"
    assert res_drag["action"] == "drag"
    assert res_drag["start_x"] == 100
    assert res_drag["end_x"] == 500

    # 4. Type
    res_type = await tool.type("pytest run")
    assert res_type["status"] == "ok"
    assert res_type["action"] == "type"
    assert res_type["length"] == 10

    # 5. Key
    res_key = await tool.key("Return", action="press")
    assert res_key["status"] == "ok"
    assert res_key["action"] == "key"
    assert res_key["key"] == "Return"

    await tool.close()
    await daemon.stop()


@pytest.mark.asyncio
async def test_computer_tool_screenshot_and_redaction(tmp_path: Path):
    sample_png = create_test_png(20, 20, color_type=6, fill_color=(255, 255, 255, 255))
    daemon = MockComputerDaemon(sample_png=sample_png)
    await daemon.start()

    tool = ComputerTool(host=daemon.host, port=daemon.port)

    # Raw screenshot without redaction
    raw_shot = await tool.screenshot()
    assert raw_shot.startswith(b"\x89PNG\r\n\x1a\n")

    # Screenshot with redaction boxes
    boxes = [(2, 2, 6, 6)]
    redacted_shot = await tool.screenshot(redaction_boxes=boxes)
    assert redacted_shot.startswith(b"\x89PNG\r\n\x1a\n")

    _, _, _, rows = decode_png_pixels(redacted_shot)
    # Masked region
    assert list(rows[3][3 * 4 : 3 * 4 + 4]) == [0, 0, 0, 255]
    # Unmasked region
    assert list(rows[0][0:4]) == [255, 255, 255, 255]

    # Capture redacted screenshot and save to disk
    save_file = tmp_path / "artifacts" / "screen.png"
    save_file.parent.mkdir(parents=True, exist_ok=True)
    saved_bytes = await tool.capture_redacted_screenshot(
        bounding_boxes=boxes, save_path=save_file
    )
    assert save_file.exists()
    assert save_file.read_bytes() == saved_bytes

    await tool.close()
    await daemon.stop()


@pytest.mark.asyncio
async def test_computer_tool_pipelined_stream_burst():
    daemon = MockComputerDaemon()
    await daemon.start()

    client = AsyncComputerClient(host=daemon.host, port=daemon.port)
    tool = ComputerTool(daemon_client=client)

    # Rapid sequential operations over persistent connection
    for i in range(25):
        resp = await tool.move(i * 10, i * 10)
        assert resp["x"] == i * 10

    assert len(daemon.received_commands) == 25
    await tool.close()
    await daemon.stop()


@pytest.mark.asyncio
async def test_computer_tool_daemon_error():
    daemon = MockComputerDaemon()
    await daemon.start()

    client = AsyncComputerClient(host=daemon.host, port=daemon.port)
    with pytest.raises(RuntimeError, match="Method not found"):
        await client.send_command("unsupported_method")

    await client.close()
    await daemon.stop()


@pytest.mark.asyncio
async def test_computer_client_auto_reconnect_after_server_disconnect():
    """Verifies that AsyncComputerClient resets state on connection drop and auto-reconnects."""
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
    client = AsyncComputerClient(host="127.0.0.1", port=port, timeout=2.0)

    try:
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
    finally:
        await client.close()
        server.close()
        await server.wait_closed()


@pytest.mark.asyncio
async def test_computer_client_read_timeout_cleanup():
    """Verifies that AsyncComputerClient resets state on read timeout."""
    async def handle_client(reader: asyncio.StreamReader, writer: asyncio.StreamWriter):
        line = await reader.readline()
        if line:
            await asyncio.sleep(5.0)

    server = await asyncio.start_server(handle_client, "127.0.0.1", 0)
    port = server.sockets[0].getsockname()[1]
    client = AsyncComputerClient(host="127.0.0.1", port=port, timeout=0.1)

    try:
        with pytest.raises(asyncio.TimeoutError):
            await client.send_command("hang_method")

        assert client.is_connected is False
        assert client._writer is None
        assert client._reader is None
    finally:
        await client.close()
        server.close()
        await server.wait_closed()


@pytest.mark.asyncio
async def test_computer_tool_high_level_resilience():
    """Verifies that high-level ComputerTool survives transient disconnection."""
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
                # Sever connection on 2nd command
                writer.close()
                await writer.wait_closed()
                break
            resp = {"jsonrpc": "2.0", "id": req["id"], "result": {"status": "ok", "action": req["method"]}}
            writer.write(json.dumps(resp).encode("utf-8") + b"\n")
            await writer.drain()

    server = await asyncio.start_server(handle_client, "127.0.0.1", 0)
    port = server.sockets[0].getsockname()[1]
    client = AsyncComputerClient(host="127.0.0.1", port=port, timeout=2.0)
    tool = ComputerTool(daemon_client=client)

    try:
        # First call succeeds
        res1 = await tool.move(10, 20)
        assert res1["status"] == "ok"

        # Second call fails due to disconnect
        with pytest.raises(ConnectionResetError):
            await tool.move(30, 40)

        # Third call transparently reconnects and succeeds
        res3 = await tool.move(50, 60)
        assert res3["status"] == "ok"
    finally:
        await tool.close()
        server.close()
        await server.wait_closed()

