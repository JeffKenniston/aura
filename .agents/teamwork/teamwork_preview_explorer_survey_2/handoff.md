# Handoff Report: Requirement R2 (Sandboxed GUI Automation & Visual Inspection with Redaction)

**Agent**: `teamwork_preview_explorer_survey_2`  
**Working Directory**: `/home/jeff/aura/.agents/teamwork/teamwork_preview_explorer_survey_2`  
**Date**: 2026-10-11  
**Handoff Type**: Hard (Task Complete)

---

## 1. Observation

1. **`deploy/images/vsock-screenshot-daemon/Cargo.toml:1-15`**:
   The guest daemon has only 3 dependencies: `x11rb = "0.14"`, `png = "0.18"`, `vsock = "0.5"`. It lacks `serde`, `serde_json`, and `base64`.
2. **`deploy/images/vsock-screenshot-daemon/src/server.rs:9-43`**:
   `handle_client_stream` accepts a stream and immediately calls `capture_screen(display_name)?` without reading client requests, writes the PNG bytes to the stream, and exits. It possesses zero JSON-RPC parsing or synthetic event dispatching logic.
3. **`deploy/images/vsock-screenshot-daemon/src/capture.rs:81` & Runtime Test**:
   Executing the compiled daemon (`target/debug/vsock-screenshot-daemon -o /tmp/test_screen.png --display :0`) produced verbatim error:
   `[vsock-screenshot-daemon] Capture failed: X11 protocol error: Reply error for image: X11 error X11Error { error_kind: Match, error_code: 8, sequence: 2, bad_value: 1080, minor_opcode: 0, major_opcode: 73, extension_name: None, request_name: Some("GetImage") }`
   Root cause: Line 81 passes `!0` (`0xFFFFFFFF`) as `plane_mask`. According to the X11 Protocol Specification: *"If drawable is a Window, the plane_mask must have 0 for all bits not in the depth of drawable, otherwise a BadMatch error results."* On 24-bit root windows, bits 24..31 must be 0 (`(1u32 << screen.root_depth) - 1` = `0x00FFFFFF`).
4. **`aura-core/src/hypervisor/firecracker.rs:16, 315-338`**:
   - `DEFAULT_SCREENSHOT_VSOCK_PORT` is set to `5252`, whereas `deploy/images/build_computer_image.sh:21` and `vsock-screenshot-daemon/src/main.rs:26` default to `5254`.
   - `process_and_save_screenshot` validates PNG magic bytes (`extract_png_bytes`) and writes to `artifacts_dir`, but does not support visual redaction masks or bounding boxes.
   - `CdpVsockForwarder` exists for browser CDP port 9222, but no corresponding `ComputerVsockForwarder` exists for the GUI daemon port.
5. **`aura-sdk/aura_sdk/tools/computer.py:1-10`**:
   Only contains a 10-line stub:
   ```python
   class ComputerTool:
       def __init__(self, transport: ZenohClient):
           self.transport = transport

       async def click(self, x: int, y: int) -> str:
           return await self.transport.dispatch_tool("computer", {"x": x, "y": y})
   ```
   No `move`, `drag`, `type`, `key`, `screenshot`, or redaction methods exist.
6. **Existing Test Suite Baseline**:
   - `cargo test --manifest-path deploy/images/vsock-screenshot-daemon/Cargo.toml`: 1 passed (`test_encode_png_dummy_buffer`).
   - `cargo test -p aura-core --lib`: 126 passed, 0 failed.
   - `/home/jeff/aura/aura-sdk/.venv/bin/pytest aura-sdk/tests`: 84 passed, 0 failed.
   - `/home/jeff/aura/aura-sdk/.venv/bin/ruff check aura-sdk/aura_sdk aura-sdk/tests`: All checks passed.
   - No tests exist for `ComputerTool` in `aura-sdk/tests/`.

---

## 2. Logic Chain

1. **Daemon Gap (Observation 1 & 2)**:
   Because `vsock-screenshot-daemon` does not parse input and immediately writes a screenshot on connection before closing, it cannot fulfill Requirement R2 ("Guest vsock daemon receiving and acknowledging JSON-RPC input actions (`click`, `move`, `type`, `key`, `drag`) ... without dropping events").
   *Remediation Required*: Add `serde`, `serde_json`, and a new `input.rs` module. Refactor `server.rs` into a persistent line-based JSON-RPC reader loop while preserving backward-compatible raw screenshot streaming.
2. **X11 Protocol Defect (Observation 3)**:
   The `BadMatch` error during screen capture directly prevents screen capture on conformant X11 servers with depth < 32.
   *Remediation Required*: Clamp `plane_mask` to `(1u32 << screen.root_depth) - 1` when `screen.root_depth < 32`.
3. **Redaction Mechanism (Observation 4 & 5)**:
   Requirement R2 specifies host-side redaction masks over specified sensitive bounding boxes before storing artifacts or presenting to model context. In `aura-sdk`, external image libraries like Pillow are not installed.
   *Remediation Required*: Implement a self-contained, pure-Python PNG scanline parser using standard library `zlib` and `struct`. Decompress `IDAT` chunks, set pixel bytes inside bounding boxes to blackout masks `(0, 0, 0)`, and re-compress into a valid PNG.
4. **Python SDK Gap (Observation 5)**:
   `ComputerTool` must expose async methods for mouse actions (`click`, `move`, `drag`), typing (`type`), key actions (`key`), and screenshot capture (`screenshot`, `capture_redacted_screenshot`).
   *Remediation Required*: Expand `aura-sdk/aura_sdk/tools/computer.py` to provide these async methods, supporting direct daemon connections (`AsyncComputerClient`) with fallback to Zenoh transport.
5. **Quality Gate Compliance (Observation 6)**:
   New unit and integration tests must be created in `deploy/images/vsock-screenshot-daemon/` (testing JSON-RPC parsing, execution, and non-dropping event queues) and `aura-sdk/tests/test_computer_tool.py` (testing all async actions, burst event continuity, and visual redaction fidelity) to satisfy acceptance criteria without regressions.

---

## 3. Caveats

1. **Hardware / Display Availability**: In CI or headless test environments without X11 or `/dev/kvm`, tests must not hang or fail. Both the daemon input handler and the Python SDK tests must utilize mockable/synthetic transports or emulator fixtures (matching the pattern in `test_browser_tool.py`).
2. **Port Uniformity**: The default port disparity (5252 in `firecracker.rs` vs 5254 in `build_computer_image.sh` and `vsock-screenshot-daemon`) should be explicitly harmonized or made dynamically configurable.

---

## 4. Conclusion

Requirement R2 is thoroughly surveyed with all code paths, protocol schemas, latent defects, and file boundaries identified.
To achieve production-grade completion of Requirement R2:
1. Update `deploy/images/vsock-screenshot-daemon/`:
   - Fix `plane_mask` in `capture.rs`.
   - Add `serde`/`serde_json` to `Cargo.toml`.
   - Implement `src/input.rs` for JSON-RPC actions (`click`, `move`, `type`, `key`, `drag`, `screenshot`).
   - Implement persistent event loop in `src/server.rs`.
2. Update `aura-core/src/hypervisor/firecracker.rs`:
   - Add redaction masks support to `process_and_save_screenshot`.
   - Reconcile vsock port configurations.
3. Update `aura-sdk/aura_sdk/tools/computer.py`:
   - Implement async `click`, `move`, `drag`, `type`, `key`, `screenshot`, and `capture_redacted_screenshot` with pure Python zero-dependency PNG scanline redaction.
4. Add comprehensive test coverage:
   - `deploy/images/vsock-screenshot-daemon/` input tests.
   - `aura-sdk/tests/test_computer_tool.py` testing all methods and redaction verification.

---

## 5. Verification Method

To independently verify the survey findings and eventual implementation:

1. **Inspect Existing Code Locations**:
   - `view_file` on `deploy/images/vsock-screenshot-daemon/src/server.rs:9-43` (verifies absence of JSON-RPC).
   - `view_file` on `deploy/images/vsock-screenshot-daemon/src/capture.rs:81` (verifies `!0` `plane_mask`).
   - `view_file` on `aura-sdk/aura_sdk/tools/computer.py:1-10` (verifies 10-line stub).

2. **Verify Daemon Test Suite**:
   ```bash
   cargo test --manifest-path deploy/images/vsock-screenshot-daemon/Cargo.toml
   ```

3. **Verify Core Hypervisor Test Suite**:
   ```bash
   cargo test -p aura-core --jobs 1
   ```

4. **Verify SDK Pytest Suite**:
   ```bash
   /home/jeff/aura/aura-sdk/.venv/bin/pytest aura-sdk/tests
   ```

5. **Verify SDK Linting**:
   ```bash
   /home/jeff/aura/aura-sdk/.venv/bin/ruff check aura-sdk/aura_sdk aura-sdk/tests
   ```
