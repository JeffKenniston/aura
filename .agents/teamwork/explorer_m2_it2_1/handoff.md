# Hard Handoff Report — Socket Read Timeout Remediation for Milestone 2 Iteration 2

**Author**: `explorer_m2_it2_1` (teamwork_preview_explorer)  
**Recipient**: `parent` (orchestrator ID: `1732b97a-66c1-4a05-9f2b-afc669944583`), `worker_m2_1`  
**Milestone**: Milestone 2 Iteration 2 (Sandboxed GUI Automation & Visual Redaction)  
**Date**: 2026-10-11T23:10:00Z  
**Type**: Hard Handoff  

---

## 1. Observation

1. **`deploy/images/vsock-screenshot-daemon/src/server.rs:126` and `191`**:
   The daemon listener functions configure a 150ms socket read timeout on accepted streams:
   ```rust
   let _ = stream.set_read_timeout(Some(Duration::from_millis(150)));
   ```
   This configuration applies to both `run_vsock_listener` and `run_tcp_listener`.

2. **`deploy/images/vsock-screenshot-daemon/src/server.rs:64-77`**:
   Inside `handle_client_interactive`:
   ```rust
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
   ```
   The `?` operator immediately propagates `std::io::ErrorKind::WouldBlock` / `TimedOut` errors out of `handle_client_interactive`.

3. **Empirical Reproduction of Socket Drop**:
   Running a client against `vsock-screenshot-daemon --tcp-listen 59999`:
   - Sent command 1: `{"jsonrpc": "2.0", "id": 1, "method": "move", "params": {"x": 10, "y": 10}}\n`
   - Received response 1: `{"jsonrpc":"2.0","id":1,"result":{"action":"move","status":"ok","x":10,"y":10}}`
   - Client slept for 250ms (`time.sleep(0.25)`).
   - Sent command 2: `{"jsonrpc": "2.0", "id": 2, ...}\n`
   - Daemon stderr output:
     ```
     [vsock-screenshot-daemon] Error handling stream: Resource temporarily unavailable
     ```
   - Client response 2: `''` (EOF, connection closed by daemon).

4. **Missing Timeout in Existing Test Suites**:
   In `deploy/images/vsock-screenshot-daemon/tests/test_daemon.rs:11-16` and `tests/empirical_stress_test.rs:11-21`:
   ```rust
   if let Ok((mut stream, _)) = listener.accept() {
       let clone = stream.try_clone().unwrap();
       let mut reader = BufReader::new(clone);
       handle_client_interactive(&mut reader, &mut stream, None, false, "e2e-client").unwrap();
   }
   ```
   `listener.accept()` never invoked `stream.set_read_timeout(Some(Duration::from_millis(150)))`, leaving the test socket with an infinite timeout and masking the bug during Milestone 2 Iteration 1.

5. **Rust Standard Library `BufRead::read_line` Partial Read Safety**:
   Tested with standalone Rust binary emitting `{"` on read 1, `WouldBlock` on read 2, and `test"}\n` on read 3:
   ```
   Timed out with partial line: {"
   Read full line (len 7): {"test"}
   ```
   `BufRead::read_line` preserves partially read bytes upon `WouldBlock` and appends incoming bytes upon subsequent reads without data loss or corruption.

6. **Preservation of Legacy Fallback Probe**:
   In `server.rs:46-99`:
   Initial `read_res` at line 46 times out after 150ms when a client connects and sends 0 bytes.
   Lines 87–97 catch `WouldBlock` / `TimedOut` and call:
   ```rust
   send_raw_screenshot(writer, display_name, length_prefix)
   ```
   Tested empirically: Client connecting and idling triggers `send_raw_screenshot` successfully.

---

## 2. Logic Chain

1. From **Observation 1 & 2**, the daemon sets a 150ms read timeout to detect legacy one-shot screenshot clients. However, once a line-delimited JSON-RPC session begins, the socket's timeout remains at 150ms.
2. From **Observation 3**, when an agent or client is idle for >150ms between actions, the OS kernel's `SO_RCVTIMEO` timer expires. `read_line` returns `ErrorKind::WouldBlock` (OS error 11: `Resource temporarily unavailable`). Because line 65 uses `while reader.read_line(&mut line)? > 0`, the `?` operator aborts the worker thread, dropping the socket and severing the connection.
3. From **Observation 4**, existing tests did not catch this because test harnesses bypassed `stream.set_read_timeout`, operating with an infinite timeout.
4. From **Observation 5**, catching `WouldBlock` and `TimedOut` in the persistent command loop and executing `continue` preserves any partial line data in `line` and resumes kernel wait without data corruption or memory leaks.
5. In comparison to disabling the timeout via `stream.set_read_timeout(None)`:
   - `handle_client_interactive<R: BufRead, W: Write>` is generic. Neither `BufRead` nor `Write` exposes socket timeout methods.
   - Modifying signatures or adding custom trait bounds would break existing callers, including tests using `Cursor<Vec<u8>>` and `Vec<u8>`.
   - In contrast, handling `WouldBlock`/`TimedOut` in the loop requires **zero signature changes**, is 100% backward compatible, and introduces negligible (<0.01%) CPU overhead (~6 kernel scheduler wake-ups per second per connection).
6. From **Observation 6**, because the legacy connection probe occurs at line 46 before entering the JSON-RPC block, looping on `WouldBlock` in the persistent loop (lines 64–77) leaves the legacy fallback path (lines 87–97) completely undisturbed.

---

## 3. Caveats

1. The 150ms timeout loop means a connected worker thread will wake up in the kernel scheduler approximately 6 times per second during idle periods. For standard microVM deployments (1–5 active agent connections), this CPU overhead is indistinguishable from zero.
2. `handle_client_interactive` does not enforce an upper bound on total session duration; an agent connection can remain open indefinitely until the client closes the socket (EOF) or an unrecoverable network error occurs.
3. Client reconnection resilience in `aura-sdk/aura_sdk/tools/computer.py` requires using an internal `_close_locked(self)` helper to avoid self-deadlocking Python's non-reentrant `asyncio.Lock`.

---

## 4. Conclusion

1. The root cause of premature JSON-RPC session termination is the propagation of `WouldBlock` via `?` in `server.rs:65`.
2. The recommended, drop-in remediation is to replace `while reader.read_line(&mut line)? > 0` with a `loop` that matches `Ok(0) => break`, processes lines on `Ok(_)`, and calls `continue` on `Err(e) if e.kind() == WouldBlock || e.kind() == TimedOut`.
3. Legacy one-shot capture fallback on initial probe is fully preserved.
4. The companion test `test_tcp_spaced_commands_succeed_over_same_connection` in `test_daemon.rs` must configure `stream.set_read_timeout(Some(Duration::from_millis(150)))` to guard against regressions.
5. The ready-to-apply patch is stored at:
   `/home/jeff/aura/.agents/teamwork/explorer_m2_it2_1/server_remediation.patch`
   The complete replacement file is stored at:
   `/home/jeff/aura/.agents/teamwork/explorer_m2_it2_1/proposed_server.rs`
   The full strategy report is stored at:
   `/home/jeff/aura/.agents/teamwork/explorer_m2_it2_1/strategy_report.md`

---

## 5. Verification Method

To verify the remediation independently:

1. **Verify Daemon Tests and Paced Connection Persistence**:
   ```bash
   cargo test --manifest-path deploy/images/vsock-screenshot-daemon/Cargo.toml
   ```
   Confirm that all 31+ unit and integration tests pass, including `test_tcp_spaced_commands_succeed_over_same_connection`.

2. **Verify Python SDK Reconnect and Redaction**:
   ```bash
   /home/jeff/aura/aura-sdk/.venv/bin/pytest aura-sdk/tests
   /home/jeff/aura/aura-sdk/.venv/bin/ruff check aura-sdk/aura_sdk aura-sdk/tests
   ```
   Confirm 98+ passed unit tests and 0 ruff lint violations.

3. **Verify Core Microkernel Regressions**:
   ```bash
   cargo test -p aura-core --jobs 1
   ```
   Confirm 305+ unit and E2E tests pass with 0 failures.

4. **Invalidation Condition**:
   If a client sleeping for 300ms between JSON-RPC commands receives EOF (`''`) or crashes with `ConnectionResetError` when communicating with the daemon running with `--tcp-listen` or `--listen`, the fix is invalidated.
