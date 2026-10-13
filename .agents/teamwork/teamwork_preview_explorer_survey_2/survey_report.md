# Comprehensive Survey Report: Requirement R2 (Sandboxed GUI Automation & Visual Inspection with Redaction)

**Surveyor**: `teamwork_preview_explorer_survey_2`  
**Date**: 2026-10-11  
**Milestone**: M5 (Hardening) — Requirement R2  
**Target Subsystems**:
- Guest VSOCK Daemon: `deploy/images/vsock-screenshot-daemon/`
- MicroVM RootFS Image Builder: `deploy/images/build_computer_image.sh`
- Hypervisor Host Supervisor: `aura-core/src/hypervisor/firecracker.rs`
- Python SDK GUI Tool: `aura-sdk/aura_sdk/tools/computer.py`
- Test Suites: `deploy/images/vsock-screenshot-daemon/`, `aura-core/`, `aura-sdk/tests/`

---

## 1. Executive Summary

Requirement R2 for Phase M5 (Hardening) specifies sandboxed GUI automation and visual inspection within an isolated Firecracker microVM. This requires:
1. An in-guest VSOCK daemon that receives and acknowledges JSON-RPC input actions (`click`, `move`, `type`, `key`, `drag`) in the X11 virtual display environment without dropping events.
2. Capturing PNG screen frames over VSOCK.
3. Host-side redaction masks applied over specified sensitive bounding boxes/patterns before storing artifacts or presenting images to model context.
4. A Python SDK `ComputerTool` exposing async methods for mouse actions, typing, key events, drag events, and redacted screenshot capture with 100% test coverage.

**Current State**:
- **Guest Daemon**: Currently only implements a one-way capture-and-stream loop on connection connect. It completely lacks JSON-RPC protocol handling and synthetic input event processing. Additionally, a latent protocol bug in `capture.rs` causes `GetImage` to fail with `BadMatch` on standard 24-bit root windows.
- **MicroVM Image**: RootFS builder installs `Xvfb`, `fluxbox`, `xcalc`, and `xdotool`. The daemon runs on AF_VSOCK port 5254 inside the guest.
- **Hypervisor Host (`firecracker.rs`)**: Implements Zero-Trust PNG magic byte validation (`extract_png_bytes`) and stores screenshot artifacts. However, it lacks host-side visual redaction masks and does not currently have a dedicated `ComputerVsockForwarder` (analogous to `CdpVsockForwarder`).
- **Python SDK (`computer.py`)**: A minimal 10-line stub with only a single `click(x, y)` method dispatching to Zenoh. No mouse movement, dragging, keyboard typing, key presses, screenshot capture, or visual redaction mechanisms exist.
- **Tests**: Zero tests exist for `ComputerTool` in `aura-sdk/tests/`, and only a single unit test exists in `vsock-screenshot-daemon`.

---

## 2. Requirement R2 Decomposition & Acceptance Criteria

### R2 Requirements Matrix
| Requirement Component | Current Status | Target Behavior |
| :--- | :--- | :--- |
| **JSON-RPC Input Actions** (`click`, `move`, `type`, `key`, `drag`) | **Missing** | Guest daemon parses JSON-RPC 2.0 requests over VSOCK/TCP and dispatches synthetic input in X11 virtual display. |
| **Event Loss Prevention** | **Missing** | Buffered line-based reader maintaining persistent stream loop; processes bursts sequentially without dropping events. |
| **PNG Capture over VSOCK** | **Partial** | Daemon captures X11 frame and transmits PNG bytes. Needs integration into JSON-RPC `screenshot` method alongside legacy binary mode. |
| **Host-Side Redaction Masks** | **Missing** | Host (or SDK) masks sensitive bounding boxes (`x, y, w, h`) and patterns directly on PNG scanlines before storage or model exposure. |
| **Python SDK `ComputerTool`** | **Partial (10% stub)** | Expose async `click`, `move`, `drag`, `type`, `key`, `screenshot`, and `capture_redacted_screenshot` methods. |
| **Workspace Verification** | **Missing tests** | Clean passes for `cargo test --manifest-path deploy/images/vsock-screenshot-daemon/Cargo.toml`, `cargo test -p aura-core`, `pytest aura-sdk/tests`, and `ruff`. |

---

## 3. Forensic Codebase Audit

### 3.1. Guest Daemon: `deploy/images/vsock-screenshot-daemon`

#### File: `Cargo.toml`
- **Location**: `deploy/images/vsock-screenshot-daemon/Cargo.toml:1-15`
- **Current Dependencies**:
  ```toml
  [dependencies]
  x11rb = "0.14"
  png = "0.18"
  vsock = "0.5"
  ```
- **Findings**:
  - Missing `serde` and `serde_json` for JSON-RPC 2.0 deserialization and response serialization.
  - Missing `base64` crate if screenshots are encoded within JSON-RPC results (both `serde` and `base64` are already present in Cargo cache).

#### File: `src/main.rs` (Lines 1-258)
- **Current Functionality**:
  - Defines CLI flags: `--listen`, `--port <PORT>` (default 5254), `--connect-host <PORT>`, `--output <FILE>`, `--stdout`, `--display <DISPLAY>`, `--length-prefix`, `--tcp-listen <PORT>`, `--tcp-connect <ADDR>`.
  - Dispatches to `server::run_vsock_listener` or `server::run_tcp_listener`.
- **Gaps**:
  - Does not initialize or reference any input handling module.

#### File: `src/capture.rs` (Lines 1-176)
- **Current Functionality**:
  - Connects to X11 via `x11rb::connect(display_name)`.
  - Obtains root window geometry and captures raw framebuffer using `conn.get_image(ImageFormat::Z_PIXMAP, root, 0, 0, width, height, !0)`.
  - Encodes RGBA pixels to PNG using `png::Encoder`.
- **Latent Bug Discovered**:
  - **Line 81**: `conn.get_image(ImageFormat::Z_PIXMAP, root, 0, 0, width, height, !0)`
  - **Error observed in test**: `X11 protocol error: Reply error for image: X11 error X11Error { error_kind: Match, error_code: 8, request_name: Some("GetImage") }`
  - **Root Cause**: X11 Protocol Specification mandates: *“If drawable is a Window, the plane_mask must have 0 for all bits not in the depth of drawable, otherwise a BadMatch error results.”* On 24-bit root windows, passing `!0` (`0xFFFFFFFF`) has bits 24..31 set, which triggers an immediate `BadMatch` (error code 8) on standard X11 servers.
  - **Fix**: Compute `plane_mask = if screen.root_depth < 32 { (1u32 << screen.root_depth) - 1 } else { !0 };`.

#### File: `src/server.rs` (Lines 1-141)
- **Current Functionality**:
  - `handle_client_stream<S: Read + Write>(mut stream: S, display_name: Option<&str>, length_prefix: bool, peer_name: &str)`
  - Reads zero bytes from client, immediately executes `capture_screen(display_name)?`, writes PNG bytes, and closes the connection.
  - Comment at line 18 notes: `// Check if client sent an initial command with a non-blocking / peek attempt (If none sent, proceed directly to screen capture)`.
- **Gaps**:
  - Never reads or processes JSON-RPC commands.
  - Drops connection immediately after sending single screenshot.
  - Unable to receive sequential input actions.

---

### 3.2. RootFS Image Environment: `deploy/images/build_computer_image.sh`

- **Packages Pre-installed** (Lines 131-156, 196-212):
  - Alpine/Debian: `xvfb`, `fluxbox`, `openbox`, `xcalc`, `xdotool`, `xterm`, `libx11`, `libxcb`, font packages.
  - `xdotool` is explicitly installed in the guest microVM filesystem.
- **Desktop Environment Initialization** (Lines 238-278):
  - Starts `Xvfb :0 -screen 0 1024x768x24 -ac +extension RANDR +extension RENDER -noreset &`
  - Starts `fluxbox &`
  - Starts `/usr/local/bin/vsock-screenshot-daemon --listen --port 5254 --display :0 &`
- **Helper Scripts** (Lines 282-298):
  - `/usr/local/bin/launch-xcalc`: Launches GUI calculator on `DISPLAY=:0`.
  - `/usr/local/bin/take-screenshot`: Invokes daemon in one-shot CLI mode.

---

### 3.3. Hypervisor Host: `aura-core/src/hypervisor/firecracker.rs`

- **Port Constants & Config** (Lines 15-26, 99-147):
  - `DEFAULT_SCREENSHOT_VSOCK_PORT = 5252` (Discrepancy: `build_computer_image.sh` and daemon default to `5254`).
  - `SpecializedRootfs::Computer`: 2 vCPUs, 1024 MiB RAM, `computer.ext4`.
- **Screenshot Processing** (Lines 315-338):
  ```rust
  pub async fn process_and_save_screenshot(
      buffer: &[u8],
      artifacts_dir: &Path,
  ) -> Result<ScreenshotArtifact, Box<dyn std::error::Error + Send + Sync>>
  ```
  - Validates `extract_png_bytes(buffer)`.
  - Saves file as `screenshot_<timestamp>_<uuid>.png`.
  - **Gap**: Does not accept or apply visual redaction masks before saving.
- **Vsock Forwarder Mechanism** (Lines 716-822):
  - `CdpVsockForwarder` implements host-to-guest port proxying over Firecracker UDS via `CONNECT <port>\n`.
  - **Gap**: No `ComputerVsockForwarder` exists for forwarding host TCP connections to the guest's screenshot/GUI daemon port.

---

### 3.4. Python SDK: `aura-sdk/aura_sdk/tools/computer.py`

- **Current Implementation** (Lines 1-10):
  ```python
  from aura_sdk.transport import ZenohClient

  class ComputerTool:
      def __init__(self, transport: ZenohClient):
          self.transport = transport

      async def click(self, x: int, y: int) -> str:
          return await self.transport.dispatch_tool("computer", {"x": x, "y": y})
  ```
- **Gaps**:
  1. Missing mouse methods: `move(x, y)`, `drag(start_x, start_y, end_x, end_y, button=1)`.
  2. Missing keyboard methods: `type(text)`, `key(key)`.
  3. Missing screenshot methods: `screenshot(...)`, `capture_redacted_screenshot(...)`.
  4. Missing visual redaction engine: No bounding box masking over sensitive areas.
  5. Missing client connector: Only accepts `ZenohClient`, lacking direct daemon connection (`AsyncComputerClient` or TCP forwarder client) with transport fallback.

---

## 4. Detailed Gap Analysis & Requirements Mapping

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│ Requirement R2 Components                                                              │
├────────────────────────────────┬──────────────────────┬────────────────────────────────┤
│ Component                      │ Existing State       │ Missing Work                   │
├────────────────────────────────┼──────────────────────┼────────────────────────────────┤
│ 1. Guest vsock JSON-RPC daemon │ One-shot PNG stream  │ JSON-RPC 2.0 parser & loop     │
│                                │ on connection        │ click, move, type, key, drag   │
│                                │                      │ event queue / no dropped msgs  │
│                                │                      │ fix X11 BadMatch plane_mask    │
├────────────────────────────────┼──────────────────────┼────────────────────────────────┤
│ 2. Capturing PNG screen frames │ capture_screen()     │ Expose via JSON-RPC method     │
│                                │ works for X11        │ support raw stream + RPC mode  │
├────────────────────────────────┼──────────────────────┼────────────────────────────────┤
│ 3. Host-side redaction masks   │ None                 │ Pure Python / Rust PNG scanline│
│                                │                      │ bounding box masking (blackout)│
│                                │                      │ pattern & bbox redaction API   │
├────────────────────────────────┼──────────────────────┼────────────────────────────────┤
│ 4. Python SDK ComputerTool     │ 10-line stub (click) │ Async methods: move, drag,     │
│                                │                      │ type, key, screenshot, redact  │
│                                │                      │ Client protocol handler        │
├────────────────────────────────┼──────────────────────┼────────────────────────────────┤
│ 5. Test suites                 │ 1 daemon test        │ Complete test_computer_tool.py │
│                                │ 0 SDK computer tests │ vsock daemon input unit tests  │
│                                │                      │ Redaction fidelity tests       │
└────────────────────────────────┴──────────────────────┴────────────────────────────────┘
```

---

## 5. Interface Contracts & Protocol Specifications

### 5.1. JSON-RPC 2.0 Protocol Schema (Guest Daemon)

The daemon communicates over line-delimited JSON-RPC 2.0 (`\n` separated messages).

#### Request Schemas:
1. **`click`**:
   ```json
   {
     "jsonrpc": "2.0",
     "id": 1,
     "method": "click",
     "params": {
       "x": 250,
       "y": 180,
       "button": 1,
       "clicks": 1
     }
   }
   ```
   *Response*:
   ```json
   {
     "jsonrpc": "2.0",
     "id": 1,
     "result": {
       "status": "ok",
       "action": "click",
       "x": 250,
       "y": 180,
       "button": 1,
       "clicks": 1
     }
   }
   ```

2. **`move`**:
   ```json
   {
     "jsonrpc": "2.0",
     "id": 2,
     "method": "move",
     "params": {
       "x": 400,
       "y": 300
     }
   }
   ```
   *Response*:
   ```json
   {
     "jsonrpc": "2.0",
     "id": 2,
     "result": {
       "status": "ok",
       "action": "move",
       "x": 400,
       "y": 300
     }
   }
   ```

3. **`type`**:
   ```json
   {
     "jsonrpc": "2.0",
     "id": 3,
     "method": "type",
     "params": {
       "text": "echo 'Hello Aura!'"
     }
   }
   ```
   *Response*:
   ```json
   {
     "jsonrpc": "2.0",
     "id": 3,
     "result": {
       "status": "ok",
       "action": "type",
       "length": 18
     }
   }
   ```

4. **`key`**:
   ```json
   {
     "jsonrpc": "2.0",
     "id": 4,
     "method": "key",
     "params": {
       "key": "Return"
     }
   }
   ```
   *Response*:
   ```json
   {
     "jsonrpc": "2.0",
     "id": 4,
     "result": {
       "status": "ok",
       "action": "key",
       "key": "Return"
     }
   }
   ```

5. **`drag`**:
   ```json
   {
     "jsonrpc": "2.0",
     "id": 5,
     "method": "drag",
     "params": {
       "start_x": 100,
       "start_y": 100,
       "end_x": 500,
       "end_y": 300,
       "button": 1
     }
   }
   ```
   *Response*:
   ```json
   {
     "jsonrpc": "2.0",
     "id": 5,
     "result": {
       "status": "ok",
       "action": "drag",
       "start_x": 100,
       "start_y": 100,
       "end_x": 500,
       "end_y": 300
     }
   }
   ```

6. **`screenshot`**:
   ```json
   {
     "jsonrpc": "2.0",
     "id": 6,
     "method": "screenshot",
     "params": {
       "format": "png"
     }
   }
   ```
   *Response*:
   ```json
   {
     "jsonrpc": "2.0",
     "id": 6,
     "result": {
       "status": "ok",
       "format": "png",
       "width": 1024,
       "height": 768,
       "data": "<base64_encoded_png_bytes>"
     }
   }
   ```

#### Stream Protocol & Event Continuity:
- Connection operates in a persistent streaming loop (`BufReader::lines()`).
- Multiple consecutive requests sent over the same connection are queued and executed in FIFO order without disconnection.
- Legacy backward compatibility: If an incoming connection disconnects without sending JSON data or sends an immediate EOF/probe, the daemon captures and transmits raw PNG bytes (with length-prefix if `--length-prefix` is set), matching `aura-core`'s existing `process_and_save_screenshot` tests.

---

### 5.2. Host-Side Redaction Engine Specification

#### Bounding Box Model:
```python
class BoundingBox(NamedTuple):
    x: int
    y: int
    width: int
    height: int
    label: str = "sensitive"
    color: tuple[int, int, int] = (0, 0, 0)  # RGB blackout
```

#### Pure Python In-Memory PNG Redaction Architecture:
Because `aura-sdk` must execute reliably without heavy binary dependencies (Pillow is not installed in `.venv`), redaction is implemented using Python's standard library `struct`, `zlib`, and `binascii`:
1. **Header Validation**: Verify 8-byte PNG signature `\x89PNG\r\n\x1a\n`.
2. **Chunk Extraction**: Parse `IHDR`, `IDAT`, `IEND`.
3. **Decompression**: Concat `IDAT` slices and decompress via `zlib.decompress()`.
4. **Scanline Manipulation**: Unpack scanlines (filter byte + row pixels). For each coordinate `(x, y)` inside any redaction bounding box, overwrite the RGB(A) bytes with the blackout mask `(0, 0, 0)`.
5. **Re-encoding**: Re-encode scanlines (using filter 0 / None) and recompress via `zlib.compress()`.
6. **Reconstruction**: Emit updated `IHDR`, new `IDAT`, and `IEND` with computed CRC32 checksums.
7. **Pattern Redaction**: Accepts regex patterns for text detection if OCR metadata is available or pre-defined visual regions (e.g., credentials header, terminal prompt, auth tokens).

---

### 5.3. Python SDK `ComputerTool` API Contract

```python
class ComputerTool:
    def __init__(
        self,
        transport: ZenohClient | None = None,
        daemon_client: AsyncComputerClient | None = None,
        host: str = "127.0.0.1",
        port: int = 5254,
    ) -> None:
        ...

    async def click(
        self, x: int, y: int, button: int = 1, clicks: int = 1
    ) -> dict[str, Any]:
        """Sends synthetic mouse click to the microVM display."""

    async def move(self, x: int, y: int) -> dict[str, Any]:
        """Moves cursor to virtual display coordinate (x, y)."""

    async def drag(
        self, start_x: int, start_y: int, end_x: int, end_y: int, button: int = 1
    ) -> dict[str, Any]:
        """Performs click-and-drag from start to end coordinates."""

    async def type(self, text: str) -> dict[str, Any]:
        """Types string sequence into active window."""

    async def key(self, key: str) -> dict[str, Any]:
        """Presses specific key or key combination (e.g. Return, Escape, ctrl+c)."""

    async def screenshot(
        self,
        redaction_boxes: list[dict[str, Any] | BoundingBox] | None = None,
        redact_patterns: list[str] | None = None,
    ) -> bytes:
        """Captures microVM display PNG and applies host-side redaction masks."""

    async def capture_redacted_screenshot(
        self,
        bounding_boxes: list[dict[str, Any]] | None = None,
        save_path: str | Path | None = None,
    ) -> bytes:
        """Captures redacted screenshot and optionally persists to artifact path."""

    async def close(self) -> None:
        """Closes active network connections."""
```

---

## 6. Latent Bugs and Architectural Findings

### 6.1. X11 Protocol Violation in `capture.rs`
- **Location**: `deploy/images/vsock-screenshot-daemon/src/capture.rs:81`
- **Current Code**:
  ```rust
  let img = conn.get_image(ImageFormat::Z_PIXMAP, root, 0, 0, width, height, !0)
  ```
- **Bug**: On a 24-bit root window, `!0` sets plane mask bits above bit 23. The X11 server immediately fails with `X11Error { error_kind: Match, error_code: 8 }` (`BadMatch`).
- **Remediation**:
  ```rust
  let plane_mask = if screen.root_depth < 32 {
      (1u32 << screen.root_depth) - 1
  } else {
      !0
  };
  let img = conn.get_image(ImageFormat::Z_PIXMAP, root, 0, 0, width, height, plane_mask)
  ```

### 6.2. Port Discrepancy Between Hypervisor and Images
- `aura-core/src/hypervisor/firecracker.rs:16`: `pub const DEFAULT_SCREENSHOT_VSOCK_PORT: u32 = 5252;`
- `deploy/images/build_computer_image.sh:21`: `VSOCK_PORT=5254`
- `deploy/images/vsock-screenshot-daemon/src/main.rs:26`: `port: 5254`
- **Remediation**: Reconcile default ports or ensure both 5252 and 5254 are supported and configurable across configurations.

### 6.3. Test Portability Outside KVM / X11
- Inside developer or CI environments, KVM may be nested or missing, and active X11 displays might not be present during headless unit tests.
- **Remediation**:
  - `vsock-screenshot-daemon`: The input handler must cleanly support headless/simulated execution when X11 is unreachable, allowing unit tests in `cargo test --manifest-path deploy/images/vsock-screenshot-daemon/Cargo.toml` to assert JSON-RPC parsing, validation, and serialization with 0 external requirements.
  - `ComputerTool` in Python: Provide an in-process TCP/socket test emulator fixture (analogous to `cdp_emulator` in `test_browser_tool.py`) enabling authentic network validation of `ComputerTool` without requiring an active Firecracker microVM.

---

## 7. Implementation Blueprint & File Boundaries

### File Change Boundaries:

```
aura/
├── deploy/images/vsock-screenshot-daemon/
│   ├── Cargo.toml               # Add serde, serde_json, base64
│   ├── src/
│   │   ├── main.rs              # Wire input module, preserve CLI flags
│   │   ├── capture.rs           # Fix X11 plane_mask BadMatch calculation
│   │   ├── input.rs             # [NEW] JSON-RPC request/response models & input actions
│   │   └── server.rs            # Line-delimited JSON-RPC loop + legacy fallback stream
│
├── aura-core/
│   └── src/hypervisor/
│       └── firecracker.rs       # Redaction masks support in process_and_save_screenshot
│                                # Optional ComputerVsockForwarder / port reconciliation
│
└── aura-sdk/
    ├── aura_sdk/
    │   └── tools/
    │       ├── computer.py      # Full ComputerTool + AsyncComputerClient + PNG redaction
    │       └── __init__.py      # Export ComputerTool and BoundingBox
    └── tests/
        └── test_computer_tool.py # [NEW] Comprehensive unit & integration tests
```

---

## 8. Verification Plan & Quality Gates

The implementation must pass all four workspace verification gates:

1. **Daemon Cargo Test**:
   ```bash
   cargo test --manifest-path deploy/images/vsock-screenshot-daemon/Cargo.toml
   ```
   *Assertions*:
   - Unit tests verify JSON-RPC request deserialization and response generation for `click`, `move`, `type`, `key`, `drag`, `screenshot`.
   - Sequential batch execution asserts zero event loss across pipelined commands.
   - PNG encoding and `plane_mask` logic verified.

2. **Aura-Core Test Suite**:
   ```bash
   cargo test -p aura-core --jobs 1
   ```
   *Assertions*:
   - 126+ tests pass with zero regressions.
   - Screenshot listener, zero-trust validation, and artifact saving pass.

3. **Python SDK Pytest**:
   ```bash
   /home/jeff/aura/aura-sdk/.venv/bin/pytest aura-sdk/tests
   ```
   *Assertions*:
   - All existing 84 tests pass.
   - New `test_computer_tool.py` suite passes 100% (mouse actions, typing, key actions, drag, raw screenshot, redacted screenshot with pixel masking, burst continuity, and fallback).

4. **Python Lint & Style**:
   ```bash
   /home/jeff/aura/aura-sdk/.venv/bin/ruff check aura-sdk/aura_sdk aura-sdk/tests
   ```
   *Assertions*:
   - Zero errors, zero warnings.
