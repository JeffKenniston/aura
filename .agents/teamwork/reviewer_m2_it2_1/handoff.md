# Review and Adversarial Audit Report — Milestone 2 Iteration 2

**Reviewer**: `reviewer_m2_it2_1` (teamwork_preview_reviewer)  
**Roles**: reviewer, critic  
**Target Milestone**: Milestone 2 Iteration 2 (Sandboxed GUI Automation Remediation)  
**Parent Orchestrator ID**: `1732b97a-66c1-4a05-9f2b-afc669944583`  
**Date**: 2026-10-11T23:25:00Z  
**Verdict**: **APPROVE**

---

## 1. Observation

Direct code inspections and verification outputs:

1. **Persistent Connection Event Loop in `vsock-screenshot-daemon` (`deploy/images/vsock-screenshot-daemon/src/server.rs:65-88`)**:
   ```rust
   // Persistent command loop over same connection
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
           Err(e)
               if e.kind() == std::io::ErrorKind::WouldBlock
                   || e.kind() == std::io::ErrorKind::TimedOut =>
           {
               continue;
           }
           Err(e) => return Err(Box::new(e)),
       }
   }
   ```
   - Confirmed: `WouldBlock` and `TimedOut` errors result in `continue`, allowing client idle intervals (> 150ms) to persist without dropping the connection.
   - Confirmed: `Ok(0)` (EOF) executes `break`, cleanly exiting the loop and returning `Ok(())`.

2. **Bounded Click Logic in `vsock-screenshot-daemon` (`deploy/images/vsock-screenshot-daemon/src/input.rs:142-198`)**:
   ```rust
   pub fn click(&self, params: &ClickParams) -> Result<Value, String> {
       let safe_clicks = params.clicks.clamp(1, 25);
       if let Ok((conn, screen_num)) = x11rb::connect(self.display_name.as_deref()) {
           // ...
           for _ in 0..safe_clicks {
               let _ = conn.xtest_fake_input(x11rb::protocol::xproto::BUTTON_PRESS_EVENT, params.button, 0, root, 0, 0, 0);
               let _ = conn.xtest_fake_input(x11rb::protocol::xproto::BUTTON_RELEASE_EVENT, params.button, 0, root, 0, 0, 0);
           }
           let _ = conn.flush();
       } else {
           let mut cmd = Command::new("xdotool");
           // ...
           let _ = cmd
               .args(["mousemove", "--", &params.x.to_string(), &params.y.to_string()])
               .args(["click", "--repeat", &safe_clicks.to_string(), &params.button.to_string()])
               .output();
       }
       Ok(serde_json::json!({
           "status": "ok",
           "action": "click",
           "x": params.x,
           "y": params.y,
           "button": params.button,
           "clicks": safe_clicks
       }))
   }
   ```
   - Confirmed: `params.clicks` is clamped to `[1, 25]` across both XTest and `xdotool` fallback branches, and `safe_clicks` is returned in the JSON-RPC response.
   - Unit test `test_click_bounding_clamps_extremes` (lines 556-573) verifies clamping for 0 clicks (`-> 1`), 10,000,000 clicks (`-> 25`), and 5 clicks (`-> 5`).

3. **Spaced Paced Inter-Command Test (`deploy/images/vsock-screenshot-daemon/tests/test_daemon.rs:123-194`)**:
   - `test_tcp_spaced_commands_succeed_over_same_connection`: sets a 150ms read timeout on the stream, transmits command 1 (`move`), sleeps 300ms, transmits command 2 (`click`), asserts `bytes_read > 0` and correct result, sleeps 300ms, and transmits command 3 (`screenshot`), verifying that all commands succeed over the single connection.

4. **Deadlock-Free Cleanup & Auto-Reconnection in Python SDK (`aura-sdk/aura_sdk/tools/computer.py:249-318`)**:
   ```python
   @property
   def is_connected(self) -> bool:
       if self._writer is None or self._reader is None:
           return False
       return not (self._writer.is_closing() or self._reader.at_eof())

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

   async def send_command(self, method: str, params: dict[str, Any] | None = None) -> dict[str, Any]:
       async with self._lock:
           try:
               if not self.is_connected:
                   await self._connect_locked()
               # ... send & read
           except Exception:
               await self._close_locked()
               raise
   ```
   - Confirmed: `_close_locked()` closes and resets `_writer` and `_reader` to `None` without re-locking `self._lock`, avoiding deadlocks in non-reentrant `asyncio.Lock`.
   - Confirmed: `is_connected` accurately reports `False` when handles are `None`, `_writer.is_closing()`, or `_reader.at_eof()`.
   - Confirmed: `send_command()` invokes `await self._close_locked()` on transport exceptions, leaving the client in a disconnected state so the next call cleanly invokes `_connect_locked()`.

5. **Empirical Test Suite Execution Results**:
   - `cargo test --manifest-path deploy/images/vsock-screenshot-daemon/Cargo.toml`:
     - 15 passed in `src/lib.rs`
     - 15 passed in `src/main.rs`
     - 5 passed in `tests/empirical_stress_test.rs`
     - 3 passed in `tests/test_daemon.rs`
     - Total: **38 passed; 0 failed**.
   - `cargo test -p aura-core --jobs 1`:
     - 135 unit tests passed
     - 170 e2e tests passed
     - Total: **305 passed; 0 failed**.
   - `cargo test -p aura-core --test m5_e2e_opaque_test`:
     - Total: **170 passed; 0 failed**.
   - `/home/jeff/aura/aura-sdk/.venv/bin/pytest aura-sdk/tests`:
     - Total: **99 passed, 1 warning in 9.17s**.
   - `/home/jeff/aura/aura-sdk/.venv/bin/ruff check aura-sdk/aura_sdk aura-sdk/tests`:
     - Total: **All checks passed!**.
   - `cargo test --manifest-path tools/ephemeral-tunnel/Cargo.toml`:
     - Total: **13 passed; 0 failed**.

---

## 2. Logic Chain

1. **Daemon Event Loop Resilience**:
   - The daemon's 150ms read timeout previously broke long-lived agent sessions during idle periods. By handling `WouldBlock` and `TimedOut` with `continue`, the daemon remains listening for the next incoming command without terminating the thread or dropping the TCP/vsock socket.
   - EOF (`Ok(0)`) is retained as the clean session exit condition, preventing zombie threads.

2. **DoS Protection via Click Bounding**:
   - Clamping `clicks` to `[1, 25]` eliminates CPU starvation from excessive synthetic events (e.g. $10^7$ clicks), while preserving flexibility for normal click patterns (single, double, triple clicks).
   - Returning `safe_clicks` maintains transparency for callers regarding the executed action.

3. **Client Deadlock Elimination & Reconnection**:
   - Refactoring cleanup into `_close_locked()` allows safe connection resets during active calls while holding `self._lock`.
   - Wrapping transport calls in `try...except Exception:` with `_close_locked()` ensures that server disconnects, network drops, or timeouts immediately reset internal state (`_writer = None`, `_reader = None`, `is_connected = False`).
   - On the subsequent command invocation, `is_connected` evaluates to `False`, triggering automatic re-establishment via `_connect_locked()`.
   - Application-level errors (e.g. unknown method) are parsed after the network `try...except`, ensuring healthy connections are not prematurely closed.

4. **Integrity Validation**:
   - No hardcoded test outputs, stubs, mocks, or facades were found in the source code.
   - All tests execute actual network sockets, JSON-RPC parsing, PNG encoding/decoding, and unfiltering logic.

---

## 3. Adversarial Challenges & Stress Testing

### Challenge Summary
- **Overall risk assessment**: **LOW**

### Challenge 1: Partial-Packet Fragment Idle Interruption
- **Assumption challenged**: Does `BufReader::read_line` preserve partially received bytes across `WouldBlock`/`TimedOut` intervals?
- **Analysis & Testing**: `BufReader` retains buffered data across non-fatal I/O errors. When `read_line(&mut line)` is called again after `continue`, it appends incoming bytes to `line` without discarding prior fragments. Once `\n` is read, the complete line is parsed.
- **Pass/Fail**: **PASS**.

### Challenge 2: Re-entrant Deadlock Under High-Concurrency SDK Calls
- **Assumption challenged**: Can concurrent coroutines calling `send_command` cause deadlocks or handle corruption on socket disconnect?
- **Analysis & Testing**: `AsyncComputerClient._lock` serializes all `send_command`, `connect`, and `close` invocations. During a disconnect, `_close_locked()` is called while holding `self._lock`, avoiding re-entrant acquisition attempts. Other queued coroutines acquire the lock only after cleanup completes, observing `is_connected == False` and triggering clean reconnection.
- **Pass/Fail**: **PASS**.

### Challenge 3: Extreme Parameter Injection
- **Assumption challenged**: Can coordinates outside display boundaries crash X11 or xdotool?
- **Analysis & Testing**: In `input.rs`, coordinates are clamped: `params.x.clamp(-32768, 32767) as i16`. Negative coordinates and coordinates up to 4K are tested in `test_boundary_coordinates_zero_negative_intmax` and `test_b_f6_01_negative_coordinates_in_move`.
- **Pass/Fail**: **PASS**.

---

## 4. Caveats

- No caveats. All 6 test suites execute cleanly and pass without regressions.

---

## 5. Conclusion

The Milestone 2 Iteration 2 implementation in `vsock-screenshot-daemon` and `aura-sdk` completely resolves the connection persistence issue, bounds synthetic click counts safely, eliminates client lock deadlocks, and provides resilient auto-reconnection. All objective verification gates pass 100%.

**Verdict**: **APPROVE**

---

## 6. Verification Method

To independently verify the implementation:

1. **Vsock Screenshot Daemon Tests**:
   ```bash
   cargo test --manifest-path deploy/images/vsock-screenshot-daemon/Cargo.toml
   ```
   *Expected*: 38 passed, 0 failed.

2. **Aura Core Test Suites**:
   ```bash
   cargo test -p aura-core --jobs 1
   cargo test -p aura-core --test m5_e2e_opaque_test
   ```
   *Expected*: 305 passed, 0 failed.

3. **Aura SDK Tests & Linter**:
   ```bash
   /home/jeff/aura/aura-sdk/.venv/bin/pytest aura-sdk/tests
   /home/jeff/aura/aura-sdk/.venv/bin/ruff check aura-sdk/aura_sdk aura-sdk/tests
   ```
   *Expected*: 99 passed in pytest; all checks passed in ruff.

4. **Ephemeral Tunnel Suite**:
   ```bash
   cargo test --manifest-path tools/ephemeral-tunnel/Cargo.toml
   ```
   *Expected*: 13 passed, 0 failed.
