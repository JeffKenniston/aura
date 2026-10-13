"""
Aura Sandboxed Computer GUI Tool (TOOL-CMP-001, TOOL-CMP-002, TOOL-CMP-003).

Provides synthetic mouse actions, keyboard events, and screen capture with
zero-dependency pure Python PNG scanline visual redaction.
"""

from __future__ import annotations

import asyncio
import base64
import json
import logging
import struct
import zlib
from pathlib import Path
from typing import Any

from aura_sdk.transport import ZenohClient

logger = logging.getLogger(__name__)

PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


def _make_chunk(chunk_type: bytes, chunk_data: bytes) -> bytes:
    """Constructs a binary PNG chunk with length, type, data, and CRC32."""
    length = len(chunk_data)
    crc = zlib.crc32(chunk_type + chunk_data) & 0xFFFFFFFF
    return struct.pack(">I4s", length, chunk_type) + chunk_data + struct.pack(">I", crc)


def mask_image_bytes(
    png_bytes: bytes,
    bounding_boxes: list[dict[str, Any] | tuple[int, int, int, int]] | None = None,
) -> bytes:
    """
    Applies pure Python scanline visual redaction over sensitive bounding boxes in PNG.

    Parses PNG chunks (IHDR, IDAT, IEND), decompresses scanlines via zlib,
    unfilters rows across standard filter modes (None, Sub, Up, Average, Paeth),
    blacks out sensitive pixel regions (x, y, w, h), re-compresses scanlines,
    updates chunk CRC32 checksums, and returns valid PNG bytes.
    """
    if not png_bytes.startswith(PNG_SIGNATURE):
        raise ValueError("Invalid PNG signature: buffer does not start with standard PNG magic bytes")

    # 1. Parse PNG chunks
    idx = len(PNG_SIGNATURE)
    ihdr_data: bytes | None = None
    idat_parts: list[bytes] = []
    other_chunks_before_idat: list[tuple[bytes, bytes]] = []
    other_chunks_after_idat: list[tuple[bytes, bytes]] = []
    seen_idat = False

    while idx < len(png_bytes):
        if idx + 8 > len(png_bytes):
            raise ValueError("Truncated PNG chunk header")
        chunk_len, chunk_type = struct.unpack(">I4s", png_bytes[idx : idx + 8])
        idx += 8

        if idx + chunk_len + 4 > len(png_bytes):
            raise ValueError(f"Truncated PNG chunk payload for {chunk_type!r}")
        chunk_data = png_bytes[idx : idx + chunk_len]
        idx += chunk_len

        chunk_crc = struct.unpack(">I", png_bytes[idx : idx + 4])[0]
        idx += 4

        expected_crc = zlib.crc32(chunk_type + chunk_data) & 0xFFFFFFFF
        if chunk_crc != expected_crc:
            raise ValueError(f"CRC32 mismatch in PNG chunk {chunk_type!r}: expected {expected_crc:#x}, found {chunk_crc:#x}")

        if chunk_type == b"IHDR":
            ihdr_data = chunk_data
        elif chunk_type == b"IDAT":
            seen_idat = True
            idat_parts.append(chunk_data)
        elif chunk_type == b"IEND":
            break
        else:
            if not seen_idat:
                other_chunks_before_idat.append((chunk_type, chunk_data))
            else:
                other_chunks_after_idat.append((chunk_type, chunk_data))

    if ihdr_data is None:
        raise ValueError("Missing IHDR chunk in PNG image")
    if not idat_parts:
        raise ValueError("Missing IDAT chunk in PNG image")

    # 2. Extract IHDR dimensions and pixel format
    width, height, bit_depth, color_type, _comp, _filt, interlace = struct.unpack(
        ">IIBBBBB", ihdr_data
    )
    if bit_depth != 8:
        raise ValueError(f"Unsupported bit depth for scanline redaction: {bit_depth}")
    if interlace != 0:
        raise ValueError("Interlaced PNG scanlines are not supported for redaction")

    if color_type == 6:  # RGBA
        bpp = 4
    elif color_type == 2:  # RGB
        bpp = 3
    elif color_type == 0:  # Grayscale
        bpp = 1
    elif color_type == 4:  # Grayscale + Alpha
        bpp = 2
    elif color_type == 3:  # Indexed color
        bpp = 1
    else:
        raise ValueError(f"Unsupported PNG color type: {color_type}")

    stride = width * bpp

    # 3. Decompress IDAT payload
    decompressed = zlib.decompress(b"".join(idat_parts))
    expected_len = height * (1 + stride)
    if len(decompressed) != expected_len:
        raise ValueError(
            f"Decompressed PNG payload length mismatch: got {len(decompressed)}, expected {expected_len}"
        )

    # 4. Unfilter scanlines into reconstructed rows
    recon_rows: list[bytearray] = []
    pos = 0
    for y in range(height):
        filter_type = decompressed[pos]
        pos += 1
        scanline = decompressed[pos : pos + stride]
        pos += stride

        recon = bytearray(stride)
        prev_recon = recon_rows[y - 1] if y > 0 else None

        if filter_type == 0:  # None
            recon[:] = scanline
        elif filter_type == 1:  # Sub
            for i in range(stride):
                left = recon[i - bpp] if i >= bpp else 0
                recon[i] = (scanline[i] + left) & 0xFF
        elif filter_type == 2:  # Up
            for i in range(stride):
                above = prev_recon[i] if prev_recon is not None else 0
                recon[i] = (scanline[i] + above) & 0xFF
        elif filter_type == 3:  # Average
            for i in range(stride):
                left = recon[i - bpp] if i >= bpp else 0
                above = prev_recon[i] if prev_recon is not None else 0
                recon[i] = (scanline[i] + ((left + above) >> 1)) & 0xFF
        elif filter_type == 4:  # Paeth
            for i in range(stride):
                left = recon[i - bpp] if i >= bpp else 0
                above = prev_recon[i] if prev_recon is not None else 0
                upper_left = (
                    prev_recon[i - bpp] if (prev_recon is not None and i >= bpp) else 0
                )
                p = left + above - upper_left
                pa = abs(p - left)
                pb = abs(p - above)
                pc = abs(p - upper_left)
                if pa <= pb and pa <= pc:
                    pr = left
                elif pb <= pc:
                    pr = above
                else:
                    pr = upper_left
                recon[i] = (scanline[i] + pr) & 0xFF
        else:
            raise ValueError(f"Unrecognized PNG filter type: {filter_type}")

        recon_rows.append(recon)

    # 5. Apply blackout redaction on specified bounding boxes
    if bounding_boxes:
        for box in bounding_boxes:
            if isinstance(box, dict):
                bx = int(box.get("x", 0))
                by = int(box.get("y", 0))
                bw = int(box.get("width", box.get("w", 0)))
                bh = int(box.get("height", box.get("h", 0)))
            elif isinstance(box, (tuple, list)) and len(box) >= 4:
                bx, by, bw, bh = int(box[0]), int(box[1]), int(box[2]), int(box[3])
            else:
                continue

            if bw <= 0 or bh <= 0:
                continue

            x0 = max(0, min(bx, width))
            y0 = max(0, min(by, height))
            x1 = max(0, min(bx + bw, width))
            y1 = max(0, min(by + bh, height))

            for r in range(y0, y1):
                row = recon_rows[r]
                for c in range(x0, x1):
                    offset = c * bpp
                    if bpp == 4:  # RGBA
                        row[offset] = 0  # Red
                        row[offset + 1] = 0  # Green
                        row[offset + 2] = 0  # Blue
                        row[offset + 3] = 255  # Solid Alpha
                    elif bpp == 3:  # RGB
                        row[offset] = 0
                        row[offset + 1] = 0
                        row[offset + 2] = 0
                    elif bpp == 1:  # Grayscale / Indexed
                        row[offset] = 0
                    elif bpp == 2:  # Grayscale + Alpha
                        row[offset] = 0
                        row[offset + 1] = 255

    # 6. Re-encode scanlines (Filter 0: None) and compress
    new_raw_scanlines = bytearray()
    for row in recon_rows:
        new_raw_scanlines.append(0)  # Filter 0
        new_raw_scanlines.extend(row)

    new_idat = zlib.compress(bytes(new_raw_scanlines), level=6)

    # 7. Reassemble PNG stream with valid CRCs
    output = bytearray(PNG_SIGNATURE)
    output.extend(_make_chunk(b"IHDR", ihdr_data))
    for c_type, c_data in other_chunks_before_idat:
        output.extend(_make_chunk(c_type, c_data))
    output.extend(_make_chunk(b"IDAT", new_idat))
    for c_type, c_data in other_chunks_after_idat:
        output.extend(_make_chunk(c_type, c_data))
    output.extend(_make_chunk(b"IEND", b""))

    return bytes(output)


class AsyncComputerClient:
    """
    Asynchronous JSON-RPC 2.0 client for Firecracker microVM computer daemon.
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
        return not (self._writer.is_closing() or self._reader.at_eof())

    async def connect(self) -> None:
        async with self._lock:
            await self._connect_locked()

    async def _close_locked(self) -> None:
        """Closes stream reader and writer, resetting connection state while holding lock."""
        if self._writer is not None:
            try:
                self._writer.close()
                await self._writer.wait_closed()
            except Exception:  # noqa: BLE001, S110
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


class ComputerTool:
    """
    Sandboxed GUI Automation Tool (TOOL-CMP-001, TOOL-CMP-002, TOOL-CMP-003).

    Controls virtual desktop inside Firecracker microVM over vsock JSON-RPC,
    providing mouse interaction, keyboard typing, and visual redaction.
    """

    def __init__(
        self,
        transport: ZenohClient | None = None,
        daemon_client: AsyncComputerClient | None = None,
        host: str = "127.0.0.1",
        port: int = 5254,
    ) -> None:
        self.transport = transport
        self.host = host
        self.port = port
        self._daemon_client = daemon_client

    async def _get_client(self) -> AsyncComputerClient:
        if self._daemon_client is None:
            self._daemon_client = AsyncComputerClient(host=self.host, port=self.port)
        return self._daemon_client

    async def click(
        self, x: int, y: int, button: int = 1, clicks: int = 1
    ) -> dict[str, Any]:
        """Sends synthetic mouse click at virtual display coordinates."""
        try:
            client = await self._get_client()
            return await client.send_command(
                "click", {"x": x, "y": y, "button": button, "clicks": clicks}
            )
        except Exception:
            if self.transport is not None and not getattr(self.transport, "is_mock", False):
                res = await self.transport.dispatch_tool(
                    "computer", {"action": "click", "x": x, "y": y, "button": button, "clicks": clicks}
                )
                return {"status": "ok", "action": "click", "result": res}
            raise

    async def move(self, x: int, y: int) -> dict[str, Any]:
        """Moves cursor to virtual display coordinate (x, y)."""
        try:
            client = await self._get_client()
            return await client.send_command("move", {"x": x, "y": y})
        except Exception:
            if self.transport is not None and not getattr(self.transport, "is_mock", False):
                res = await self.transport.dispatch_tool("computer", {"action": "move", "x": x, "y": y})
                return {"status": "ok", "action": "move", "result": res}
            raise

    async def drag(
        self, start_x: int, start_y: int, end_x: int, end_y: int, button: int = 1
    ) -> dict[str, Any]:
        """Performs mouse drag from start coordinates to end coordinates."""
        try:
            client = await self._get_client()
            return await client.send_command(
                "drag",
                {
                    "start_x": start_x,
                    "start_y": start_y,
                    "end_x": end_x,
                    "end_y": end_y,
                    "x": end_x,
                    "y": end_y,
                    "button": button,
                },
            )
        except Exception:
            if self.transport is not None and not getattr(self.transport, "is_mock", False):
                res = await self.transport.dispatch_tool(
                    "computer",
                    {
                        "action": "drag",
                        "start_x": start_x,
                        "start_y": start_y,
                        "end_x": end_x,
                        "end_y": end_y,
                        "button": button,
                    },
                )
                return {"status": "ok", "action": "drag", "result": res}
            raise

    async def type(self, text: str) -> dict[str, Any]:
        """Types string sequence into active window."""
        try:
            client = await self._get_client()
            return await client.send_command("type", {"text": text})
        except Exception:
            if self.transport is not None and not getattr(self.transport, "is_mock", False):
                res = await self.transport.dispatch_tool("computer", {"action": "type", "text": text})
                return {"status": "ok", "action": "type", "result": res}
            raise

    async def key(self, key: str, action: str = "press") -> dict[str, Any]:
        """Sends synthetic key event."""
        try:
            client = await self._get_client()
            return await client.send_command("key", {"key": key, "action": action})
        except Exception:
            if self.transport is not None and not getattr(self.transport, "is_mock", False):
                res = await self.transport.dispatch_tool(
                    "computer", {"action": "key", "key": key, "action_type": action}
                )
                return {"status": "ok", "action": "key", "result": res}
            raise

    async def screenshot(
        self,
        redaction_boxes: list[Any] | None = None,
    ) -> bytes:
        """Captures display screenshot PNG and applies visual redaction."""
        raw_b64 = None
        try:
            client = await self._get_client()
            res = await client.send_command("screenshot", {"format": "png"})
            raw_b64 = res.get("png_base64") or res.get("data")
        except Exception:
            if self.transport is not None and not getattr(self.transport, "is_mock", False):
                res = await self.transport.dispatch_tool("computer", {"action": "screenshot", "format": "png"})
                if isinstance(res, str):
                    raw_b64 = res
                elif isinstance(res, dict):
                    raw_b64 = res.get("png_base64") or res.get("data")
            if raw_b64 is None:
                raise

        if not raw_b64:
            raise RuntimeError("No screenshot image data returned by computer daemon")

        png_bytes = base64.b64decode(raw_b64)
        if redaction_boxes:
            png_bytes = mask_image_bytes(png_bytes, redaction_boxes)
        return png_bytes

    async def capture_redacted_screenshot(
        self,
        bounding_boxes: list[Any] | None = None,
        save_path: str | Path | None = None,
    ) -> bytes:
        """Captures redacted screenshot and optionally persists to disk."""
        png_bytes = await self.screenshot(redaction_boxes=bounding_boxes)
        if save_path is not None:
            Path(save_path).write_bytes(png_bytes)
        return png_bytes

    async def close(self) -> None:
        """Closes active network connection to daemon."""
        if self._daemon_client is not None:
            await self._daemon_client.close()
            self._daemon_client = None
