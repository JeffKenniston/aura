# Hard Handoff Report — Adversarial Review for Milestone 2

**Author**: `reviewer_m2_2` (teamwork_preview_reviewer / critic)  
**Recipient**: `orchestrator` / `worker_m2_1`  
**Milestone**: Milestone 2: Sandboxed GUI Automation & Visual Redaction (Features 7-12)  
**Date**: 2026-10-11T22:50:00Z  
**Verdict**: **REQUEST_CHANGES**

---

## Review Summary

**Verdict**: **REQUEST_CHANGES**  
**Overall Risk Assessment**: **HIGH**  
**Integrity Audit**: **PASS** (Zero integrity violations; genuine implementations with no facade stubs, mocks, or hardcoded answers).

While the implementation of X11 plane mask depth calculation, pure Python zero-dependency PNG visual redaction (`mask_image_bytes`), and JSON-RPC dispatch is mathematically sound, well-crafted, and passes all existing test suites, an adversarial stress analysis revealed a **critical architectural flaw** in `vsock-screenshot-daemon`: persistent interactive JSON-RPC sessions are dropped whenever the client takes more than 150ms between actions. This makes normal, paced multi-step GUI automation by LLM agents fail with `ConnectionResetError`.

---

## 1. Observation

1. **`deploy/images/vsock-screenshot-daemon/src/server.rs`**:
   - Lines 126 and 191 configure a read timeout of 150ms on incoming listener streams:
     ```rust
     let _ = stream.set_read_timeout(Some(Duration::from_millis(150)));
     ```
   - In `handle_client_interactive` (lines 53–76):
     ```rust
     if trimmed.starts_with('{') {
         // Line-delimited JSON-RPC mode
         if let Some(resp) = executor.process_line(trimmed) {
             let resp_str = serde_json::to_string(&resp)?;
             writer.write_all(resp_str.as_bytes())?;
             writer.write_all(b"\n")?;
             writer.flush()?;
         }
         line.clear();

         // Persistent command loop over same connection
         while reader.read_line(&mut line)? > 0 {
             let trimmed = line.trim();
             if !trimmed.is_empty() {
                 if let Some(resp) = executor.process_line(trimmed) {
                     let resp_str = serde_json::to_string(&resp)?;
                     writer.write_all(resp_str.as_bytes())?;
                     writer.write_all(b"\n")?;
                     writer.flush()?;
                 }
             }
             line.clear();
         }
         Ok(())
     }
     ```
   - The 150ms socket read timeout is never disabled or reset when entering line-delimited JSON-RPC mode.
   - The `while reader.read_line(&mut line)? > 0` loop propagates all I/O errors using `?`.
   - When a client is idle for >150ms between requests, `read_line` fails with `std::io::ErrorKind::WouldBlock` / `TimedOut`.
   - The `?` operator immediately aborts `handle_client_interactive`, the worker thread exits, and the socket is closed.

2. **Empirical Reproduction of Daemon Premature Termination**:
   - Started daemon: `cargo run --manifest-path deploy/images/vsock-screenshot-daemon/Cargo.toml -- --tcp-listen 55555`.
   - Client connected and sent request 1 (`move`). Response received: `{"jsonrpc":"2.0","id":1,"result":{"action":"move","status":"ok","x":10,"y":10}}`.
   - Client slept for 250ms (> 150ms timeout) and sent request 2 (`move`).
   - Response from daemon: `''` (EOF / closed socket).
   - Daemon printed: `[vsock-screenshot-daemon] Error handling stream: Resource temporarily unavailable`.
   - Client crashed with `ConnectionResetError`.

3. **`aura-sdk/aura_sdk/tools/computer.py`**:
   - In `AsyncComputerClient.send_command` (lines 284–294):
     ```python
     line_bytes = await asyncio.wait_for(
         self._reader.readline(), timeout=self.timeout
     )
     if not line_bytes:
         raise ConnectionResetError("Connection closed by computer daemon")
     ```
   - If the connection drops or times out, `_writer` and `_reader` remain non-None.
   - `self.is_connected` remains `True` (`self._writer is not None and not self._writer.is_closing()`).
   - Subsequent calls to `send_command` do not invoke `_connect_locked()`, repeatedly failing on the dead writer with `BrokenPipeError` or immediate EOF.

4. **`deploy/images/vsock-screenshot-daemon/src/input.rs`**:
   - In `InputExecutor::click` (line 157):
     ```rust
     for _ in 0..params.clicks.max(1) {
     ```
   - `params.clicks` is an unbounded `u32`. If a client passes e.g. `100_000_000`, the worker thread spins indefinitely in a CPU loop generating synthetic events.

5. **Test Suite Verification Results**:
   - `cargo test --manifest-path deploy/images/vsock-screenshot-daemon/Cargo.toml`: 30 passed, 0 failed.
   - `cargo test -p aura-core --jobs 1`: 135 lib tests passed, 170 e2e tests passed (0 failed).
   - `cargo test -p aura-core --test m5_e2e_opaque_test`: 170 passed, 0 failed.
   - `/home/jeff/aura/aura-sdk/.venv/bin/pytest aura-sdk/tests`: 96 passed, 0 failed in 4.00s.
   - `/home/jeff/aura/aura-sdk/.venv/bin/ruff check aura-sdk/aura_sdk aura-sdk/tests`: `All checks passed!`.

---

## 2. Logic Chain

1. **Root Cause of Session Termination**:
   - The daemon's 150ms read timeout was introduced to detect legacy one-shot clients that connect and send 0 bytes, triggering a fallback raw PNG capture.
   - However, when a client does send data and starts JSON-RPC mode, the socket's read timeout remains at 150ms.
   - In real agentic workflows, an LLM agent issues an action (e.g. `click`), waits for the environment to react or completes an LLM reasoning turn (typically taking 500ms–3000ms), and then issues the next action.
   - Because the 150ms timeout is still active on the persistent socket, `read_line` on the daemon thread times out after 150ms and aborts the connection via `?`.
   - Existing unit tests did not catch this because `test_daemon.rs` called `handle_client_interactive` directly on a stream without calling `set_read_timeout`, and Python unit tests ran against `MockComputerDaemon`.

2. **Root Cause of Client Reconnect Failure**:
   - `AsyncComputerClient` was designed assuming persistent connections, but does not close and clean up `_writer` and `_reader` upon connection exceptions.
   - Because `is_connected` remains `True`, the client never attempts to reconnect, rendering the client permanently broken after any transient disconnect.

3. **Integrity & Correctness of Other Subsystems**:
   - `capture.rs`: The plane mask formula `compute_plane_mask(depth)` correctly handles 1, 8, 16, 24, 30, and 32 bits, preventing X11 `BadMatch` errors.
   - `computer.py`: `mask_image_bytes` unfilters all 5 PNG filter modes (None, Sub, Up, Average, Paeth), blacks out sensitive pixels with solid opaque black, correctly clamps out-of-bounds/negative boxes, preserves ancillary chunks, and recomputes all chunk CRCs.
   - Error responses are properly serialized as JSON-RPC error objects with standard error codes (`-32700`, `-32601`, `-32602`, `-32603`).

---

## 3. Findings

### [Critical] Finding 1: Persistent JSON-RPC Session Drop on Inter-Command Latency (>150ms)
- **What**: The daemon closes persistent client connections if more than 150 milliseconds elapse between requests.
- **Where**: `deploy/images/vsock-screenshot-daemon/src/server.rs:126, 191` and `server.rs:65`.
- **Why**: `run_vsock_listener` and `run_tcp_listener` set a 150ms timeout on accepted streams. Inside `handle_client_interactive`, `while reader.read_line(&mut line)? > 0` propagates `WouldBlock`/`TimedOut` errors via `?`, immediately aborting the thread and closing the socket.
- **Suggestion**:
  1. In `handle_client_interactive`, do not exit on `ErrorKind::WouldBlock` or `ErrorKind::TimedOut` during the persistent command loop; instead, `continue` waiting for incoming commands:
     ```rust
     loop {
         match reader.read_line(&mut line) {
             Ok(0) => break, // EOF
             Ok(_) => {
                 let trimmed = line.trim();
                 if !trimmed.is_empty() {
                     if let Some(resp) = executor.process_line(trimmed) {
                         let resp_str = serde_json::to_string(&resp)?;
                         writer.write_all(resp_str.as_bytes())?;
                         writer.write_all(b"\n")?;
                         writer.flush()?;
                     }
                 }
                 line.clear();
             }
             Err(e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => {
                 continue;
             }
             Err(e) => return Err(Box::new(e)),
         }
     }
     ```
  2. Alternatively, disable the read timeout (`stream.set_read_timeout(None)`) once JSON-RPC mode is detected.
  3. Add an integration test in `test_daemon.rs` asserting that commands spaced 300ms apart succeed over the same connection.

### [Major] Finding 2: Missing Self-Healing Connection Cleanup in `AsyncComputerClient`
- **What**: `AsyncComputerClient` does not reset connection state upon connection loss.
- **Where**: `aura-sdk/aura_sdk/tools/computer.py:284-297`.
- **Why**: When `readline()` returns empty bytes (`ConnectionResetError`), `self._writer` and `self._reader` are not cleaned up. `self.is_connected` remains `True`, preventing reconnection on subsequent commands.
- **Suggestion**:
  Wrap the body of `send_command` in a `try...except Exception:` block that calls `await self.close()` before re-raising, ensuring defunct socket handles are discarded so the next call cleanly re-establishes the connection:
  ```python
  except Exception:
      await self.close()
      raise
  ```

### [Minor] Finding 3: Unbounded Loop on `clicks` Parameter in `InputExecutor::click`
- **What**: `clicks` parameter in `ClickParams` is looped without an upper bound.
- **Where**: `deploy/images/vsock-screenshot-daemon/src/input.rs:157`.
- **Why**: A caller passing a huge `clicks` integer (e.g. `10_000_000`) hangs the daemon thread generating synthetic events.
- **Suggestion**: Clamp `clicks` to a reasonable bound: `params.clicks.clamp(1, 25)`.

---

## 4. Adversarial Challenge Analysis

### Challenge 1: Persistent Connection Lifetime vs. Agent Latency
- **Assumption Challenged**: The daemon supports long-running interactive GUI automation over a single vsock stream.
- **Attack Scenario**: An agent clicks a button, waits for network latency or LLM reasoning (e.g., 500ms), and attempts to move the mouse or take a screenshot.
- **Actual Behavior**: The daemon thread terminates after 150ms of idle time, dropping the connection and causing the next action to fail.
- **Blast Radius**: 100% of interactive multi-turn agent sessions using `ComputerTool` fail.
- **Mitigation**: Handle `WouldBlock`/`TimedOut` in the server loop or clear the socket read timeout once JSON-RPC mode begins.

### Challenge 2: Client Connection Fault Tolerance
- **Assumption Challenged**: `ComputerTool` survives transient connection interruptions.
- **Attack Scenario**: Daemon restarts or vsock buffer truncates connection.
- **Actual Behavior**: `AsyncComputerClient` retains defunct socket references and never attempts reconnection.
- **Blast Radius**: The agent tool remains permanently non-functional for the remainder of the process lifecycle.
- **Mitigation**: Clean up connection state on error so subsequent invocations can reconnect.

---

## 5. Stress Test Results Summary

| Scenario | Expected Behavior | Actual Behavior | Result |
|---|---|---|---|
| Pipelined burst of 50 commands (0ms delay) | Process all 50 without event loss | All 50 processed in order with matching IDs | **PASS** |
| Paced interactive commands (250ms idle delay) | Process command 2 on same connection | Daemon terminates connection after 150ms | **FAIL (Finding 1)** |
| Invalid JSON-RPC syntax in stream | Return `-32700 Parse error`, keep stream open | Returns parse error and keeps stream open | **PASS** |
| Unknown JSON-RPC method | Return `-32601 Method not found`, keep stream open | Returns method not found error and continues | **PASS** |
| Out-of-bounds bounding boxes `(-100, -100, 10, 10)` in `mask_image_bytes` | Clamp safely without crash or corrupted pixels | Clamped safely, valid PNG generated | **PASS** |
| Overlapping bounding boxes in `mask_image_bytes` | Redact overlap idempotently | Opaque solid black applied, valid CRC32 | **PASS** |
| 5 standard PNG filter modes (None, Sub, Up, Average, Paeth) | Accurately reconstruct and re-encode scanlines | Reconstructed scanlines pixel-perfect | **PASS** |
| CRC32 corruption in input PNG chunk | Reject with `ValueError` | Rejected with `ValueError: CRC32 mismatch` | **PASS** |
| Client reconnect after daemon drop | Cleanly re-establish connection | Retains dead handles, fails on subsequent calls | **FAIL (Finding 2)** |
| Extreme `clicks` parameter (e.g., 100M) | Cap to safe maximum | Loops up to `u32::MAX` | **FAIL (Finding 3)** |

---

## 6. Verified Claims

- `cargo test --manifest-path deploy/images/vsock-screenshot-daemon/Cargo.toml`: 30 tests pass (`test result: ok. 14 passed; 14 passed; 2 passed; finished in 0.48s`).
- `cargo test -p aura-core --jobs 1`: 135 unit tests pass, 170 e2e tests pass.
- `cargo test -p aura-core --test m5_e2e_opaque_test`: 170 tests pass.
- `pytest aura-sdk/tests`: 96 tests pass in 4.00s.
- `ruff check aura-sdk/aura_sdk aura-sdk/tests`: All checks passed.
- `mask_image_bytes` zero-dependency visual redaction strictly preserves PNG magic header, IHDR, IDAT, IEND, and CRC checksums.
- `compute_plane_mask` correctly computes depths for 1, 8, 16, 24, 30, and 32 bits.

---

## 7. Caveats

- Hardware X11 display testing was performed using `x11rb` headless fallbacks and unit test harnesses since this host does not have an active physical GPU or running `Xvfb` display server.
- Interlaced PNGs are explicitly not supported by design (not used by `vsock-screenshot-daemon`).

---

## 8. Conclusion

Milestone 2 implementation is well-structured and contains no integrity violations. However, because of the critical 150ms socket read timeout flaw that drops persistent sessions on normal inter-command latency, and the lack of reconnection resilience in the client, the explicit verdict is **REQUEST_CHANGES**.

Once `worker_m2_1` resolves Findings 1 and 2, Milestone 2 will be robust, production-grade, and ready for approval.

---

## 9. Verification Method

To verify the remediation:

1. **Reproduction Test for Finding 1**:
   Run daemon with `--tcp-listen 55555`. Send command 1, sleep 300ms, send command 2. Verify command 2 succeeds and does not encounter a closed socket.
2. **Daemon Tests**:
   ```bash
   cargo test --manifest-path deploy/images/vsock-screenshot-daemon/Cargo.toml
   ```
3. **Aura-Core Tests**:
   ```bash
   cargo test -p aura-core --jobs 1
   ```
4. **Python SDK Tests**:
   ```bash
   /home/jeff/aura/aura-sdk/.venv/bin/pytest aura-sdk/tests
   /home/jeff/aura/aura-sdk/.venv/bin/ruff check aura-sdk/aura_sdk aura-sdk/tests
   ```
