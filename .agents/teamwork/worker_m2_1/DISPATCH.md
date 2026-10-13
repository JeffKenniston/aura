## 2026-10-11T18:59:22Z
You are worker_m2_1, a teamwork_preview_worker implementing Milestone 2: Sandboxed GUI Automation & Visual Redaction (Features 7-12) for Phase M5 (Hardening) in Aura.

Your working directory is: /home/jeff/aura/.agents/teamwork/worker_m2_1
Parent Orchestrator Conversation ID: 1732b97a-66c1-4a05-9f2b-afc669944583

MANDATORY INTEGRITY WARNING:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

MANDATORY: Read the authoritative user request first:
/home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md

Also read:
- /home/jeff/aura/PROJECT.md
- /home/jeff/aura/.agents/teamwork/teamwork_preview_explorer_survey_2/survey_report.md
- /home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-host-domain/SKILL.md
- /home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-exe-vm-domain/SKILL.md
- /home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-tool-domain/SKILL.md

File Write Ownership (Exclusive):
- deploy/images/vsock-screenshot-daemon/Cargo.toml
- deploy/images/vsock-screenshot-daemon/src/capture.rs
- deploy/images/vsock-screenshot-daemon/src/input.rs
- deploy/images/vsock-screenshot-daemon/src/server.rs
- deploy/images/vsock-screenshot-daemon/src/main.rs
- deploy/images/vsock-screenshot-daemon/src/lib.rs (if needed)
- deploy/images/vsock-screenshot-daemon/tests/
- aura-core/src/hypervisor/firecracker.rs (only for port 5254 constant harmonization)
- aura-sdk/aura_sdk/tools/computer.py
- aura-sdk/tests/test_computer_tool.py

Tasks:
1. `deploy/images/vsock-screenshot-daemon`:
   - In `Cargo.toml`: Add dependencies `serde = { version = "1.0", features = ["derive"] }`, `serde_json = "1.0"`, `base64 = "0.22"`.
   - In `src/capture.rs`: Fix `plane_mask` calculation on line 81:
     `let plane_mask = if screen.root_depth < 32 { (1u32 << screen.root_depth) - 1 } else { !0 };`
     Prevent `BadMatch` X11 protocol error on 24-bit root windows.
   - In `src/input.rs`: Implement X11 synthetic input actions (`click`, `move`, `drag`, `type`, `key`). Handle cases where X11 display may not be reachable during unit testing gracefully.
   - In `src/server.rs`: Implement persistent JSON-RPC 2.0 request loop parsing line-delimited JSON. Support methods:
     * `click` (x, y, button, clicks)
     * `move` (x, y)
     * `drag` (start_x, start_y, end_x, end_y, button)
     * `type` (text)
     * `key` (key, action)
     * `screenshot` (format: "png") -> captures frame, encodes base64, returns in JSON-RPC result.
     Preserve legacy one-shot raw PNG capture fallback for clients expecting raw binary stream.
   - Add unit tests in `deploy/images/vsock-screenshot-daemon/tests/` or in module tests verifying JSON-RPC parsing, dispatching, and response serialization.

2. `aura-core/src/hypervisor/firecracker.rs`:
   - Harmonize default port constant: `pub const DEFAULT_SCREENSHOT_VSOCK_PORT: u32 = 5254;`.

3. `aura-sdk/aura_sdk/tools/computer.py`:
   - Implement zero-dependency pure Python PNG scanline visual redaction function:
     `mask_image_bytes(png_bytes: bytes, bounding_boxes: list[dict[str, Any] | tuple[int, int, int, int]]) -> bytes`.
     Must parse PNG chunks (`IHDR`, `IDAT`, `IEND`), uncompress scanlines via `zlib`, black out sensitive pixels `(x, y, w, h)`, re-compress scanlines, update chunk CRC32, and return valid PNG bytes.
   - Implement asynchronous `ComputerTool`:
     Methods:
     * `click(self, x: int, y: int, button: int = 1, clicks: int = 1) -> dict[str, Any]`
     * `move(self, x: int, y: int) -> dict[str, Any]`
     * `drag(self, start_x: int, start_y: int, end_x: int, end_y: int, button: int = 1) -> dict[str, Any]`
     * `type(self, text: str) -> dict[str, Any]`
     * `key(self, key: str, action: str = "press") -> dict[str, Any]`
     * `screenshot(self, redaction_boxes: list[Any] | None = None) -> bytes`
     * `capture_redacted_screenshot(self, bounding_boxes: list[Any] | None = None, save_path: str | Path | None = None) -> bytes`
     * `close(self) -> None`

4. `aura-sdk/tests/test_computer_tool.py`:
   - Comprehensive test suite testing all input methods, mock daemon communication, JSON-RPC parsing, and PNG scanline visual redaction (verifying blacked out pixels, unaffected surrounding pixels, valid PNG headers and CRC).

5. Verification Commands:
   - `cargo test --manifest-path deploy/images/vsock-screenshot-daemon/Cargo.toml`
   - `cargo test -p aura-core --jobs 1`
   - `cargo test -p aura-core --test m5_e2e_opaque_test`
   - `/home/jeff/aura/aura-sdk/.venv/bin/pytest aura-sdk/tests`
   - `/home/jeff/aura/aura-sdk/.venv/bin/ruff check aura-sdk/aura_sdk aura-sdk/tests`

6. Output:
   - Update `progress.md` in your working directory.
   - Write a complete `handoff.md` in your working directory detailing changes made, test commands and passing outputs, and layout compliance.
   - Send completion message to parent with report path.
