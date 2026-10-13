## 2026-10-11T23:10:42Z
You are worker_m2_it2_1, a teamwork_preview_worker implementing Milestone 2 Iteration 2 fixes for Sandboxed GUI Automation in Aura.

Your working directory is: /home/jeff/aura/.agents/teamwork/worker_m2_it2_1
Parent Orchestrator Conversation ID: 1732b97a-66c1-4a05-9f2b-afc669944583

MANDATORY INTEGRITY WARNING:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

MANDATORY: Read the authoritative user request first:
/home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md

Also read:
- /home/jeff/aura/PROJECT.md
- /home/jeff/aura/.agents/teamwork/reviewer_m2_2/handoff.md
- /home/jeff/aura/.agents/teamwork/explorer_m2_it2_1/strategy_report.md
- /home/jeff/aura/.agents/teamwork/explorer_m2_it2_2/strategy_report.md
- /home/jeff/aura/.agents/teamwork/explorer_m2_it2_2/computer_remediation.patch
- /home/jeff/aura/.agents/teamwork/explorer_m2_it2_2/test_remediation.patch
- /home/jeff/aura/.agents/teamwork/explorer_m2_it2_3/strategy_report.md

File Write Ownership (Exclusive):
- deploy/images/vsock-screenshot-daemon/src/server.rs
- deploy/images/vsock-screenshot-daemon/src/input.rs
- deploy/images/vsock-screenshot-daemon/tests/test_daemon.rs
- aura-sdk/aura_sdk/tools/computer.py
- aura-sdk/tests/test_computer_tool.py

Tasks:
1. In `deploy/images/vsock-screenshot-daemon/src/server.rs`:
   - In `handle_client_interactive`: in the persistent JSON-RPC loop (after initial line is processed), replace the while loop with:
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
     Ok(())
     ```
   - This ensures client idle intervals (> 150ms) do NOT terminate persistent sessions.

2. In `deploy/images/vsock-screenshot-daemon/src/input.rs`:
   - Clamp `params.clicks.clamp(1, 25)` in `InputExecutor::click` for both XTest and xdotool paths, returning safe_clicks in result.
   - Add unit test `test_click_bounding_clamps_extremes` verifying 0 -> 1, 10,000,000 -> 25, and 5 -> 5.

3. In `deploy/images/vsock-screenshot-daemon/tests/test_daemon.rs`:
   - Add `test_tcp_spaced_commands_succeed_over_same_connection` (spawning a server with `stream.set_read_timeout(Some(Duration::from_millis(150)))`, connecting client, sending command 1, sleeping 300ms, sending command 2, sleeping 300ms, sending command 3, asserting all succeed over the single connection).

4. In `aura-sdk/aura_sdk/tools/computer.py`:
   - Apply the self-healing reconnection fixes from `explorer_m2_it2_2`:
     * In `is_connected`: check `_reader is not None and not _reader.at_eof() and _writer is not None and not _writer.is_closing()`.
     * Add `async def _close_locked(self) -> None`: closes `_writer` without re-locking `self._lock` (preventing deadlock with non-reentrant asyncio.Lock), sets `_writer = None`, `_reader = None`.
     * Update `close()` to call `await self._close_locked()` under `self._lock`.
     * In `_connect_locked()`: call `await self._close_locked()` before opening new connection.
     * In `send_command()`: wrap network I/O in `try...except Exception: await self._close_locked(); raise`.

5. In `aura-sdk/tests/test_computer_tool.py`:
   - Add companion tests from `explorer_m2_it2_2/test_remediation.patch` verifying that `AsyncComputerClient` recovers and reconnects after server drop or EOF.

6. Verification Commands:
   - `cargo test --manifest-path deploy/images/vsock-screenshot-daemon/Cargo.toml`
   - `cargo test -p aura-core --jobs 1`
   - `cargo test -p aura-core --test m5_e2e_opaque_test`
   - `/home/jeff/aura/aura-sdk/.venv/bin/pytest aura-sdk/tests`
   - `/home/jeff/aura/aura-sdk/.venv/bin/ruff check aura-sdk/aura_sdk aura-sdk/tests`

7. Output:
   - Update `progress.md` in your working directory.
   - Write a complete `handoff.md` in your working directory detailing changes made, test commands and passing outputs, and layout compliance.
   - Send completion message to parent with report path.
