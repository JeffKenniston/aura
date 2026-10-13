# Strategy Report: Socket Read Timeout Remediation for Milestone 2 Iteration 2

**Author**: `explorer_m2_it2_1` (teamwork_preview_explorer)  
**Recipient**: `parent` (orchestrator ID: `1732b97a-66c1-4a05-9f2b-afc669944583`), `worker_m2_1`  
**Milestone**: Milestone 2 Iteration 2 (Sandboxed GUI Automation & Visual Redaction)  
**Date**: 2026-10-11T23:05:00Z  
**Target Subsystem**: `deploy/images/vsock-screenshot-daemon/src/server.rs`  

---

## 1. Executive Summary

Milestone 2 adversarial review by `reviewer_m2_2` uncovered a critical regression in `deploy/images/vsock-screenshot-daemon/src/server.rs`:
**Persistent interactive JSON-RPC sessions are dropped whenever the client pauses for >150 milliseconds between actions.**

In real-world LLM agent automation workflows, an agent issues an action (e.g., `click`), pauses for the browser/desktop environment to render or executes an LLM reasoning step (typically 300ms to 3,000ms), and then issues the next command. Because the daemon worker thread retains an active 150ms read timeout and uses the `?` operator on `reader.read_line(&mut line)?`, any idle interval >150ms causes `std::io::ErrorKind::WouldBlock` / `TimedOut` to abort the thread, closing the socket and crashing the client with `ConnectionResetError`.

This report delivers:
1. **Root cause analysis** linking the legacy one-shot connection probe design to the interactive session drop.
2. **Empirical reproduction** confirming the socket drop and verifying that existing unit tests failed to catch it due to missing timeout configuration on test streams.
3. **Exact drop-in remediation strategy** in `handle_client_interactive` that safely handles `WouldBlock`/`TimedOut` in the persistent command loop without altering function signatures or breaking generic stream abstraction.
4. **Analysis of timeout resetting (`stream.set_read_timeout(None)`)** detailing trade-offs, coherence constraints, and why loop-level timeout handling is superior.
5. **Rigorous proof of legacy one-shot fallback preservation** for initial connection probes.
6. **Proof of partial read safety** across timeout boundaries using Rust standard library `BufRead::read_line`.
7. **Companion integration test** for `deploy/images/vsock-screenshot-daemon/tests/test_daemon.rs` asserting 300ms spaced commands over a single connection with an active 150ms read timeout.
8. **Cross-subsystem integration** harmonizing client reconnection in `computer.py` and click bounding in `input.rs`.

---

## 2. Root Cause Analysis

### 2.1 The Legacy Probe Mechanism
In `deploy/images/vsock-screenshot-daemon/src/server.rs`:
- Line 126 (`run_vsock_listener`) and line 191 (`run_tcp_listener`) configure a 150ms socket read timeout on accepted streams:
  ```rust
  let _ = stream.set_read_timeout(Some(Duration::from_millis(150)));
  ```
- This timeout was introduced to detect legacy one-shot screenshot clients (e.g., hypervisor capture probes or simple `nc`/`cat` pipes) that connect to port 5254 and send zero bytes, expecting the daemon to automatically stream raw PNG bytes.
- When a legacy client connects and sends nothing, the initial `read_line` call at line 46 times out after 150ms:
  ```rust
  Err(e)
      if e.kind() == std::io::ErrorKind::WouldBlock
          || e.kind() == std::io::ErrorKind::TimedOut =>
  {
      println!("[vsock-screenshot-daemon] Client {} timed out on initial command, sending raw frame", peer_name);
      send_raw_screenshot(writer, display_name, length_prefix)
  }
  ```
  This legacy fallback successfully serves one-shot PNG frames.

### 2.2 The Interactive Session Defect
When a JSON-RPC client connects, it sends an initial JSON command (e.g., `{"jsonrpc": "2.0", "id": 1, ...}\n`) within 150ms.
- The initial `read_line` completes with `Ok(_)`.
- `trimmed.starts_with('{')` evaluates to `true`.
- The daemon processes the command, serializes the JSON-RPC response, writes it to `writer`, and flushes.
- Then, the daemon enters the persistent loop (lines 64–76):
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

### 2.3 Why the Session Drops
1. The 150ms socket read timeout is **still active on the socket file descriptor**.
2. When the client is idle between commands for >150ms, the kernel's `SO_RCVTIMEO` timer expires.
3. `reader.read_line(&mut line)` fails with `std::io::ErrorKind::WouldBlock` (OS error 11: `EAGAIN` / `EWOULDBLOCK`).
4. Because the loop uses `reader.read_line(&mut line)?`, the `?` operator immediately converts the `WouldBlock` error to `Err(Box::new(e))` and returns from `handle_client_interactive`.
5. The spawned thread terminates, dropping `stream` and `reader`.
6. Dropping the stream closes the socket (sends TCP FIN / resets vsock connection).
7. When the client eventually sends command 2, the daemon has already closed the connection. The client receives EOF on read (`''`) and crashes with `ConnectionResetError`.

### 2.4 Why Existing Tests Did Not Detect This
In `deploy/images/vsock-screenshot-daemon/tests/test_daemon.rs` and `tests/empirical_stress_test.rs`:
```rust
let (mut stream, _) = listener.accept().unwrap();
let clone = stream.try_clone().unwrap();
let mut reader = BufReader::new(clone);
handle_client_interactive(&mut reader, &mut stream, None, false, "client").unwrap();
```
The test server accepted the connection and directly invoked `handle_client_interactive` **without ever calling `stream.set_read_timeout(...)`**. Because the test sockets had an infinite timeout (`SO_RCVTIMEO = 0`), they never generated `WouldBlock` errors regardless of delays. Meanwhile, in production, `run_vsock_listener` and `run_tcp_listener` explicitly configure 150ms timeouts on every accepted stream.

---

## 3. Empirical Reproduction & Verification

### 3.1 Live Reproduction Against Unpatched Daemon
Running against `vsock-screenshot-daemon` started with `--tcp-listen 59999`:
```python
s = socket.create_connection(('127.0.0.1', 59999))
s.sendall(b'{"jsonrpc": "2.0", "id": 1, "method": "move", "params": {"x": 10, "y": 10}}\n')
f = s.makefile('r')
print('Resp 1:', f.readline().strip())
time.sleep(0.25) # Sleep 250ms (> 150ms daemon timeout)
s.sendall(b'{"jsonrpc": "2.0", "id": 2, "method": "move", "params": {"x": 20, "y": 20}}\n')
print('Resp 2:', repr(f.readline()))
```
**Output**:
```
Resp 1: {"jsonrpc":"2.0","id":1,"result":{"action":"move","status":"ok","x":10,"y":10}}
Resp 2: ''
```
**Daemon stderr**:
`[vsock-screenshot-daemon] Error handling stream: Resource temporarily unavailable`

### 3.2 Verification of Partial Read Preservation on `WouldBlock`
A critical concern when catching `WouldBlock` inside `read_line` is whether partial data in the buffer is lost or corrupted if a network packet is delayed mid-line.
Using a standalone Rust mock reader emitting `{"` on read 1, `WouldBlock` on read 2, and `test"}\n` on read 3:
```
Timed out with partial line: {"
Read full line (len 7): {"test"}
```
**Finding**: Rust standard library's `BufRead::read_line(&mut self, buf: &mut String)` preserves any partially accumulated bytes when `WouldBlock` is returned. On the next call, it continues reading from the underlying stream and appends remaining bytes to `buf`. Therefore, handling `WouldBlock` with `continue` does **not** corrupt or lose fragmented incoming lines.

---

## 4. Remediation Strategy Formulation

### 4.1 Strategy Comparison

| Dimension | Strategy A: Loop-Level `WouldBlock`/`TimedOut` Handling (Recommended) | Strategy B: Disabling Socket Read Timeout (`set_read_timeout(None)`) |
|---|---|---|
| **Mechanism** | Replace `while reader.read_line(...)? > 0` with `loop { match reader.read_line(...) { Err(WouldBlock \| TimedOut) => continue, ... } }` | Attempt to call `stream.set_read_timeout(None)` once `trimmed.starts_with('{')` is observed |
| **Function Signature** | **Zero changes**. Remains `pub fn handle_client_interactive<R: BufRead, W: Write>(...)` | **Requires changes**. `W` and `R` are generic; neither `BufRead` nor `Write` has `set_read_timeout`. Requires adding custom trait bounds or passing callbacks |
| **Compatibility** | 100% backward compatible with all unit tests using `Cursor<Vec<u8>>` and `Vec<u8>` writers | Breaks unit tests (`Cursor` / `Vec<u8>` do not support socket options without mock trait boilerplate) |
| **Liveness & Health** | Thread wakes every 150ms in kernel scheduler (~6 Hz), providing natural checkpoints for shutdown signals | Thread blocks indefinitely in kernel until client data or EOF |
| **CPU Impact** | Negligible (<0.01% CPU). Thread sleeps in kernel `recv` with `SO_RCVTIMEO` | Zero CPU |
| **Code Risk** | Extremely low. Localized to 20 lines in `server.rs` | Moderate to high. Alters public signatures or requires trait acrobatics |

### 4.2 Primary Drop-in Formulation (Strategy A)

In `deploy/images/vsock-screenshot-daemon/src/server.rs`:
Replace lines 64–77:

```rust
<<<< PREVIOUS CODE (VULNERABLE) >>>>
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

```rust
<<<< PROPOSED DROP-IN CODE (REMEDIATED) >>>>
                // Persistent command loop over same connection
                loop {
                    match reader.read_line(&mut line) {
                        Ok(0) => break, // EOF: client cleanly closed the connection
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

### 4.3 Detailed Semantics of the Remediated Loop
1. **`Ok(0) => break`**: Clean client termination (TCP FIN or vsock disconnect). The loop terminates cleanly and returns `Ok(())`.
2. **`Ok(_) => { ... }`**: Command received. `executor.process_line` generates the JSON-RPC response, writes and flushes it, and clears `line`.
3. **`Err(e) if WouldBlock || TimedOut => continue`**: Socket read timeout elapsed while waiting for the next command. The loop does not abort; it simply continues waiting. If partial line data was received prior to timeout, it remains intact in `line` and the next read appends to it.
4. **`Err(e) => return Err(Box::new(e))`**: Unrecoverable I/O errors (e.g., `ConnectionReset`, `BrokenPipe`) immediately propagate to the caller to terminate the thread and clean up resources.

---

## 5. Preservation of Legacy Fallback Behavior

A key invariant is that the daemon's legacy one-shot raw capture mode must not be broken.

### 5.1 Legacy Code Path Tracing
When a legacy client connects:
1. **Case 1: Client connects and sends nothing**
   - The thread configures `set_read_timeout(Some(Duration::from_millis(150)))`.
   - The initial `reader.read_line(&mut line)` at line 46 is executed.
   - After 150ms of silence, the kernel returns `WouldBlock` / `TimedOut`.
   - Lines 87–97 match:
     ```rust
     Err(e)
         if e.kind() == std::io::ErrorKind::WouldBlock
             || e.kind() == std::io::ErrorKind::TimedOut =>
     {
         println!("[vsock-screenshot-daemon] Client {} timed out on initial command, sending raw frame", peer_name);
         send_raw_screenshot(writer, display_name, length_prefix)
     }
     ```
   - The raw PNG screenshot is transmitted, and the function returns `Ok(())`.
2. **Case 2: Client connects and sends a legacy text command (e.g. `screenshot\n` or `raw\n`)**
   - `read_line` completes within 150ms.
   - `trimmed.starts_with('{')` is `false`.
   - Lines 78–85 execute:
     ```rust
     println!("[vsock-screenshot-daemon] Client {} requested raw stream", peer_name);
     send_raw_screenshot(writer, display_name, length_prefix)
     ```
   - The raw PNG screenshot is transmitted.

### 5.2 Empirical Verification
We compiled and executed a verification test reproducing the exact socket flow:
```
Legacy probe result: RAW_FALLBACK_TIMEOUT
Legacy probe SUCCESS!
```
The legacy fallback probe is completely independent of the interactive persistent loop and remains 100% intact.

---

## 6. Companion Integration Test for `test_daemon.rs`

To ensure this regression never recurs and to independently verify the fix under real socket conditions, the following companion integration test must be added to `deploy/images/vsock-screenshot-daemon/tests/test_daemon.rs`.

**Key Invariant**: The test **must explicitly configure** `stream.set_read_timeout(Some(std::time::Duration::from_millis(150)))` on the accepted server stream. This accurately replicates the runtime configuration of `run_vsock_listener` and `run_tcp_listener`.

```rust
#[test]
fn test_tcp_spaced_commands_succeed_over_same_connection() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind ephemeral port");
    let addr = listener.local_addr().unwrap();

    let server_handle = thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            // Explicitly set 150ms read timeout matching daemon listener configuration
            stream
                .set_read_timeout(Some(std::time::Duration::from_millis(150)))
                .unwrap();
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
    let bytes_read = client_reader.read_line(&mut line2).unwrap();
    assert!(bytes_read > 0, "Connection dropped prematurely after 300ms idle delay");

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

## 7. Cross-Subsystem Alignment with Peer Explorer Findings

To provide the implementer (`worker_m2_1`) with a complete, coordinated remediation plan, this section synthesizes the findings from `explorer_m2_it2_2` and `explorer_m2_it2_3`:

### 7.1 Python Client Reconnect Resilience (`aura_sdk/tools/computer.py`)
- **Deadlock Warning**: In `AsyncComputerClient`, calling `await self.close()` inside `async with self._lock:` deadlocks immediately because `asyncio.Lock` is non-reentrant.
- **Solution**: Implement `_close_locked(self)` which does not acquire `self._lock`. `close()` delegates to `_close_locked()`. In `send_command`, wrap the I/O block with:
  ```python
  except Exception:
      await self._close_locked()
      raise
  ```
  This ensures that any dropped connection immediately clears `self._writer` and `self._reader`, enabling automatic reconnection on subsequent commands.

### 7.2 Input Click Bounding (`vsock-screenshot-daemon/src/input.rs`)
- In `InputExecutor::click`:
  ```rust
  let safe_clicks = params.clicks.clamp(1, 25);
  ```
  Apply `safe_clicks` to both the XTest loop and the `xdotool` fallback, preventing CPU exhaustion loops while accommodating single, double, and triple clicks.

---

## 8. Actionable Implementation Checklist for `worker_m2_1`

1. **Apply Server Fix**:
   - In `deploy/images/vsock-screenshot-daemon/src/server.rs`: Replace lines 64–77 with the remediated `loop` matching `WouldBlock` and `TimedOut` with `continue`.
2. **Apply Input Bounding**:
   - In `deploy/images/vsock-screenshot-daemon/src/input.rs`: Add `params.clicks.clamp(1, 25)` and companion unit test `test_click_bounding_clamps_extremes`.
3. **Add Companion Integration Test**:
   - In `deploy/images/vsock-screenshot-daemon/tests/test_daemon.rs`: Add `test_tcp_spaced_commands_succeed_over_same_connection`.
4. **Apply Python SDK Fixes**:
   - In `aura-sdk/aura_sdk/tools/computer.py`: Implement `_close_locked`, call it in `send_command` error handler, and update `close()`.
   - In `aura-sdk/tests/test_computer_tool.py`: Add `disconnect_next` support to `MockComputerDaemon` and companion reconnect unit tests.
5. **Run Verification Gates**:
   - `cargo test --manifest-path deploy/images/vsock-screenshot-daemon/Cargo.toml`
   - `cargo test -p aura-core --jobs 1`
   - `/home/jeff/aura/aura-sdk/.venv/bin/pytest aura-sdk/tests`
   - `/home/jeff/aura/aura-sdk/.venv/bin/ruff check aura-sdk/aura_sdk aura-sdk/tests`
