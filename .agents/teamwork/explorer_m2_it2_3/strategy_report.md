# Strategy Report: Input Bounding and Companion Test Suites for Milestone 2

**Author**: `explorer_m2_it2_3` (teamwork_preview_explorer)  
**Recipient**: `parent` (orchestrator: `1732b97a-66c1-4a05-9f2b-afc669944583`), `worker_m2_1`  
**Milestone**: Milestone 2 Iteration 2 (Sandboxed GUI Automation & Visual Redaction)  
**Date**: 2026-10-11T22:56:00Z  

---

## 1. Executive Summary

Milestone 2 review by `reviewer_m2_2` issued a **REQUEST_CHANGES** verdict due to three identified issues:
1. **Critical Flaw**: Daemon drops persistent JSON-RPC sessions whenever agent commands are spaced by >150ms because `handle_client_interactive` does not handle `ErrorKind::WouldBlock` / `ErrorKind::TimedOut` during the persistent command loop.
2. **Major Flaw**: `AsyncComputerClient` does not reset connection handles upon connection drop, permanently wedging the client on dead sockets and preventing self-healing reconnection.
3. **Minor Flaw**: `InputExecutor::click` executes an unbounded loop over `params.clicks`, creating CPU exhaustion risks if a client sends an extreme integer (e.g., $10^8$).

This report provides the complete, production-grade formulation for:
- Safe bounding of `clicks` in `deploy/images/vsock-screenshot-daemon/src/input.rs` and companion unit tests.
- A robust integration test in `deploy/images/vsock-screenshot-daemon/tests/test_daemon.rs` validating that commands spaced 300ms apart succeed over a single persistent connection with an active 150ms socket read timeout.
- A self-healing connection reset implementation in `aura-sdk/aura_sdk/tools/computer.py`—**specifically resolving a critical non-reentrant deadlock vulnerability** in `asyncio.Lock`—accompanied by companion unit tests in `aura-sdk/tests/test_computer_tool.py`.

---

## 2. Input Bounding Formulation (`vsock-screenshot-daemon/src/input.rs`)

### 2.1 Problem Analysis
In `deploy/images/vsock-screenshot-daemon/src/input.rs`:
```rust
// Current lines 157-176 and 186
for _ in 0..params.clicks.max(1) {
    let _ = conn.xtest_fake_input(...);
    let _ = conn.xtest_fake_input(...);
}
...
let _ = cmd
    .args(["mousemove", "--", &params.x.to_string(), &params.y.to_string()])
    .args(["click", "--repeat", &params.clicks.to_string(), &params.button.to_string()])
    .output();
```
- `params.clicks` is deserialized from JSON as an unbounded `u32`.
- If an agent or untrusted caller passes `clicks: 100_000_000`, the worker thread spins for hours generating synthetic X11 events, stalling the daemon and exhausting Firecracker microVM CPU.
- In the fallback branch, `xdotool` is invoked with `--repeat 100000000`, similarly hanging external process execution.
- If `clicks: 0` is sent, `0..params.clicks.max(1)` executes 1 click, but the returned JSON reports `"clicks": 0`, causing inconsistency.

### 2.2 Proposed Solution & Exact Code Snippet
Clamp `clicks` to `[1, 25]` using `params.clicks.clamp(1, 25)`. A maximum of 25 consecutive clicks accommodates standard GUI interactions (single, double, triple clicks, or fast multi-click operations) while completely preventing denial-of-service loops.

Apply to both the `x11rb` XTest loop and the `xdotool` fallback, and return `safe_clicks` in the result payload.

#### Target File: `deploy/images/vsock-screenshot-daemon/src/input.rs`
```rust
    /// Dispatches mouse click at virtual coordinates (x, y).
    pub fn click(&self, params: &ClickParams) -> Result<Value, String> {
        let safe_clicks = params.clicks.clamp(1, 25);
        if let Ok((conn, screen_num)) = x11rb::connect(self.display_name.as_deref()) {
            let setup = conn.setup();
            if let Some(screen) = setup.roots.get(screen_num) {
                let root = screen.root;
                let _ = conn.warp_pointer(
                    x11rb::NONE,
                    root,
                    0,
                    0,
                    0,
                    0,
                    params.x.clamp(-32768, 32767) as i16,
                    params.y.clamp(-32768, 32767) as i16,
                );
                for _ in 0..safe_clicks {
                    let _ = conn.xtest_fake_input(
                        x11rb::protocol::xproto::BUTTON_PRESS_EVENT,
                        params.button,
                        0,
                        root,
                        0,
                        0,
                        0,
                    );
                    let _ = conn.xtest_fake_input(
                        x11rb::protocol::xproto::BUTTON_RELEASE_EVENT,
                        params.button,
                        0,
                        root,
                        0,
                        0,
                        0,
                    );
                }
                let _ = conn.flush();
            }
        } else {
            let mut cmd = Command::new("xdotool");
            if let Some(ref disp) = self.display_name {
                cmd.env("DISPLAY", disp);
            }
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

### 2.3 Companion Unit Test
Add to `deploy/images/vsock-screenshot-daemon/src/input.rs` inside `mod tests`:
```rust
    #[test]
    fn test_click_bounding_clamps_extremes() {
        let executor = InputExecutor::default();

        // 1. Zero clicks clamps to 1
        let zero_req = r#"{"jsonrpc": "2.0", "id": 1, "method": "click", "params": {"x": 10, "y": 20, "button": 1, "clicks": 0}}"#;
        let zero_resp = executor.process_line(zero_req).expect("Failed to process zero clicks");
        assert_eq!(zero_resp.result.unwrap()["clicks"], 1);

        // 2. Huge clicks (10,000,000) clamps to 25
        let huge_req = r#"{"jsonrpc": "2.0", "id": 2, "method": "click", "params": {"x": 10, "y": 20, "button": 1, "clicks": 10000000}}"#;
        let huge_resp = executor.process_line(huge_req).expect("Failed to process huge clicks");
        assert_eq!(huge_resp.result.unwrap()["clicks"], 25);

        // 3. Normal clicks within range (5) preserved
        let normal_req = r#"{"jsonrpc": "2.0", "id": 3, "method": "click", "params": {"x": 10, "y": 20, "button": 1, "clicks": 5}}"#;
        let normal_resp = executor.process_line(normal_req).expect("Failed to process normal clicks");
        assert_eq!(normal_resp.result.unwrap()["clicks"], 5);
    }
```

---

## 3. Companion Integration Test for Persistent Paced Commands (`test_daemon.rs`)

### 3.1 Problem Analysis
In `deploy/images/vsock-screenshot-daemon/src/server.rs`:
- Line 126 and line 191 call `stream.set_read_timeout(Some(Duration::from_millis(150)))`.
- Inside `handle_client_interactive`:
  ```rust
  while reader.read_line(&mut line)? > 0 { ... }
  ```
- Because of `?`, when a client is idle for >150ms between actions, `read_line` returns `ErrorKind::WouldBlock` or `ErrorKind::TimedOut`.
- The `?` operator aborts `handle_client_interactive` with an error, the thread exits, and the socket closes.
- Existing tests in `test_daemon.rs` did NOT catch this because they called `handle_client_interactive` on raw `TcpStream` instances without calling `stream.set_read_timeout`.

### 3.2 Required Daemon Fix (`src/server.rs`)
In `deploy/images/vsock-screenshot-daemon/src/server.rs`:
Replace lines 64–77 with:
```rust
                // Persistent command loop over same connection
                loop {
                    match reader.read_line(&mut line) {
                        Ok(0) => break, // EOF: client cleanly closed connection
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
                            // Inter-command latency: client is idle between agent actions.
                            // Continue waiting for subsequent commands over persistent connection.
                            continue;
                        }
                        Err(e) => return Err(Box::new(e)),
                    }
                }
                Ok(())
```

### 3.3 Companion Integration Test Formulation
Add this integration test to `deploy/images/vsock-screenshot-daemon/tests/test_daemon.rs`.

**Key Architectural Invariant**: The test **must explicitly configure** `stream.set_read_timeout(Some(Duration::from_millis(150)))` on the server thread. Without this, the test would execute against a socket with infinite timeout and fail to exercise the idle timeout branch.

```rust
#[test]
fn test_tcp_spaced_commands_succeed_over_same_connection() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind ephemeral port");
    let addr = listener.local_addr().unwrap();

    let server_handle = thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            // Explicitly set 150ms read timeout matching daemon listener configuration
            stream.set_read_timeout(Some(std::time::Duration::from_millis(150))).unwrap();
            let clone = stream.try_clone().unwrap();
            let mut reader = BufReader::new(clone);
            handle_client_interactive(&mut reader, &mut stream, None, false, "paced-client").unwrap();
        }
    });

    let mut client = TcpStream::connect(addr).expect("Failed to connect to daemon TCP");
    let mut client_reader = BufReader::new(client.try_clone().unwrap());

    // 1. Initial command
    let cmd1 = r#"{"jsonrpc": "2.0", "id": 1, "method": "move", "params": {"x": 120, "y": 240}}"#;
    client.write_all(cmd1.as_bytes()).unwrap();
    client.write_all(b"\n").unwrap();
    client.flush().unwrap();

    let mut line1 = String::new();
    client_reader.read_line(&mut line1).unwrap();
    let resp1: serde_json::Value = serde_json::from_str(line1.trim()).unwrap();
    assert_eq!(resp1["id"], 1);
    assert_eq!(resp1["result"]["status"], "ok");
    assert_eq!(resp1["result"]["action"], "move");
    assert_eq!(resp1["result"]["x"], 120);

    // 2. Inter-command latency: sleep 300ms (> 150ms socket read timeout)
    std::thread::sleep(std::time::Duration::from_millis(300));

    // 3. Second command over the exact same connection
    let cmd2 = r#"{"jsonrpc": "2.0", "id": 2, "method": "click", "params": {"x": 120, "y": 240, "button": 1, "clicks": 2}}"#;
    client.write_all(cmd2.as_bytes()).unwrap();
    client.write_all(b"\n").unwrap();
    client.flush().unwrap();

    let mut line2 = String::new();
    client_reader.read_line(&mut line2).unwrap();
    let resp2: serde_json::Value = serde_json::from_str(line2.trim()).unwrap();
    assert_eq!(resp2["id"], 2);
    assert_eq!(resp2["result"]["status"], "ok");
    assert_eq!(resp2["result"]["action"], "click");
    assert_eq!(resp2["result"]["clicks"], 2);

    // 4. Second inter-command latency: sleep another 300ms
    std::thread::sleep(std::time::Duration::from_millis(300));

    // 5. Third command over the same connection
    let cmd3 = r#"{"jsonrpc": "2.0", "id": 3, "method": "screenshot", "params": {"format": "png"}}"#;
    client.write_all(cmd3.as_bytes()).unwrap();
    client.write_all(b"\n").unwrap();
    client.flush().unwrap();

    let mut line3 = String::new();
    client_reader.read_line(&mut line3).unwrap();
    let resp3: serde_json::Value = serde_json::from_str(line3.trim()).unwrap();
    assert_eq!(resp3["id"], 3);
    assert_eq!(resp3["result"]["status"], "ok");
    assert!(resp3["result"]["png_base64"].is_string());

    drop(client_reader);
    drop(client);
    server_handle.join().unwrap();
}
```

---

## 4. Companion Test & Reconnect Resilience (`aura-sdk/aura_sdk/tools/computer.py` & `tests/test_computer_tool.py`)

### 4.1 Deadlock Vulnerability Analysis in `AsyncComputerClient`
In `aura-sdk/aura_sdk/tools/computer.py`:
- `AsyncComputerClient` uses `self._lock = asyncio.Lock()`.
- `send_command` acquires the lock:
  ```python
  async def send_command(self, ...):
      async with self._lock:
          ...
  ```
- `close()` also acquires the lock:
  ```python
  async def close(self) -> None:
      async with self._lock:
          ...
  ```
- **CRITICAL DEADLOCK WARNING**: If `send_command` catches an exception inside `async with self._lock:` and calls `await self.close()`, **the process will deadlock immediately**! Python's `asyncio.Lock` is strictly non-reentrant.
- **Remediation**: Create a dedicated internal method `_close_locked(self)` that closes the writer without re-acquiring `self._lock`. `close()` delegates to `_close_locked()`, and the exception handler in `send_command` invokes `_close_locked()`.

### 4.2 Required Fix in `aura_sdk/tools/computer.py`
```python
    async def _close_locked(self) -> None:
        """Internal cleanup helper called when self._lock is already held."""
        if self._writer is not None:
            try:
                self._writer.close()
                await self._writer.wait_closed()
            except Exception:
                pass
            self._writer = None
            self._reader = None

    async def close(self) -> None:
        """Closes connection to daemon and resets internal state."""
        async with self._lock:
            await self._close_locked()

    async def send_command(self, method: str, params: dict[str, Any] | None = None) -> dict[str, Any]:
        """Sends a JSON-RPC 2.0 command over persistent vsock connection."""
        async with self._lock:
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
            try:
                self._writer.write(raw_line.encode("utf-8"))
                await self._writer.drain()

                line_bytes = await asyncio.wait_for(
                    self._reader.readline(), timeout=self.timeout
                )
                if not line_bytes:
                    raise ConnectionResetError("Connection closed by computer daemon")

                resp = json.loads(line_bytes.decode("utf-8").strip())
                if "error" in resp and resp["error"] is not None:
                    err = resp["error"]
                    msg = err.get("message", "Unknown error")
                    code = err.get("code", -32603)
                    raise RuntimeError(f"Computer daemon error ({code}): {msg}")

                return resp.get("result", {})
            except Exception:
                await self._close_locked()
                raise
```

### 4.3 Enhancing `MockComputerDaemon` in `test_computer_tool.py`
In `aura-sdk/tests/test_computer_tool.py`, update `MockComputerDaemon` to support simulating server disconnects:
```python
class MockComputerDaemon:
    """Mock in-process JSON-RPC 2.0 daemon for testing ComputerTool."""

    def __init__(self, sample_png: bytes | None = None):
        self.sample_png = sample_png or create_test_png(16, 16)
        self.server: asyncio.Server | None = None
        self.host = "127.0.0.1"
        self.port: int = 0
        self.received_commands: list[dict[str, Any]] = []
        self.disconnect_next: bool = False
        self.active_writers: list[asyncio.StreamWriter] = []

    async def start(self) -> None:
        self.server = await asyncio.start_server(self._handle_client, self.host, 0)
        self.port = self.server.sockets[0].getsockname()[1]

    async def _handle_client(
        self, reader: asyncio.StreamReader, writer: asyncio.StreamWriter
    ) -> None:
        self.active_writers.append(writer)
        try:
            while True:
                line = await reader.readline()
                if not line:
                    break
                if self.disconnect_next:
                    writer.close()
                    await writer.wait_closed()
                    break

                req = json.loads(line.decode("utf-8").strip())
                self.received_commands.append(req)
                ...
        finally:
            if writer in self.active_writers:
                self.active_writers.remove(writer)
```

### 4.4 Companion Tests in `test_computer_tool.py`
Add the following two tests at the end of `aura-sdk/tests/test_computer_tool.py`:

```python
@pytest.mark.asyncio
async def test_async_computer_client_reconnects_after_disconnect():
    daemon = MockComputerDaemon()
    await daemon.start()

    client = AsyncComputerClient(host=daemon.host, port=daemon.port)

    # 1. Initial command succeeds over connection 1
    resp1 = await client.send_command("move", {"x": 50, "y": 60})
    assert resp1["action"] == "move"
    assert resp1["x"] == 50
    assert client.is_connected

    # 2. Simulate server-side socket disconnect
    daemon.disconnect_next = True
    with pytest.raises(ConnectionResetError):
        await client.send_command("move", {"x": 70, "y": 80})

    # Assert connection state has been cleaned up and is not marked connected
    assert not client.is_connected

    # 3. Server back to normal; subsequent command re-establishes connection and succeeds
    daemon.disconnect_next = False
    resp2 = await client.send_command("move", {"x": 90, "y": 100})
    assert resp2["action"] == "move"
    assert resp2["x"] == 90
    assert client.is_connected

    await client.close()
    await daemon.stop()


@pytest.mark.asyncio
async def test_computer_tool_recovers_after_server_disconnect():
    daemon = MockComputerDaemon()
    await daemon.start()

    tool = ComputerTool(host=daemon.host, port=daemon.port)

    # 1. First tool action succeeds
    res1 = await tool.move(10, 20)
    assert res1["action"] == "move"

    # 2. Server drops mid-session
    daemon.disconnect_next = True
    with pytest.raises(ConnectionResetError):
        await tool.click(30, 40)

    # 3. Subsequent tool action automatically reconnects and succeeds
    daemon.disconnect_next = False
    res2 = await tool.move(50, 60)
    assert res2["action"] == "move"
    assert res2["x"] == 50

    await tool.close()
    await daemon.stop()
```

---

## 5. Implementation Roadmap for Worker

| Sequence | Target File | Action | Key Verification |
|---|---|---|---|
| 1 | `deploy/images/vsock-screenshot-daemon/src/input.rs` | Apply `clicks.clamp(1, 25)` and add unit test `test_click_bounding_clamps_extremes` | `cargo test --manifest-path deploy/images/vsock-screenshot-daemon/Cargo.toml` |
| 2 | `deploy/images/vsock-screenshot-daemon/src/server.rs` | Update persistent loop to handle `WouldBlock`/`TimedOut` with `continue` | `cargo test --manifest-path deploy/images/vsock-screenshot-daemon/Cargo.toml` |
| 3 | `deploy/images/vsock-screenshot-daemon/tests/test_daemon.rs` | Add `test_tcp_spaced_commands_succeed_over_same_connection` (300ms spacing with 150ms read timeout) | `cargo test --manifest-path deploy/images/vsock-screenshot-daemon/Cargo.toml` |
| 4 | `aura-sdk/aura_sdk/tools/computer.py` | Implement `_close_locked(self)`, update `close()`, and add error cleanup in `send_command` | `pytest aura-sdk/tests` |
| 5 | `aura-sdk/tests/test_computer_tool.py` | Add `disconnect_next` to `MockComputerDaemon` and add reconnect test functions | `pytest aura-sdk/tests/test_computer_tool.py` |
| 6 | Entire Workspace | Run full test suite and linter gates | `pytest aura-sdk/tests` (98+ passed), `ruff check`, `cargo test -p aura-core` |

---

## 6. Verification Criteria

1. **Rust Daemon Tests**:
   ```bash
   cargo test --manifest-path deploy/images/vsock-screenshot-daemon/Cargo.toml
   ```
   Must pass all tests including the new unit test in `src/input.rs` and the 300ms spaced integration test in `tests/test_daemon.rs`.
2. **Python SDK Tests**:
   ```bash
   /home/jeff/aura/aura-sdk/.venv/bin/pytest aura-sdk/tests/test_computer_tool.py
   /home/jeff/aura/aura-sdk/.venv/bin/pytest aura-sdk/tests
   /home/jeff/aura/aura-sdk/.venv/bin/ruff check aura-sdk/aura_sdk aura-sdk/tests
   ```
   Must pass all 98+ tests with 0 warnings/failures and 0 ruff lint violations.
3. **Core Repository Verification**:
   ```bash
   cargo test -p aura-core --jobs 1
   ```
   Must continue to pass with 0 failures and 0 regressions.
