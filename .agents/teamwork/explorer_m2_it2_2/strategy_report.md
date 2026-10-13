# Strategy Report: Client Connection Resilience in `AsyncComputerClient`

**Component**: `aura-sdk/aura_sdk/tools/computer.py`  
**Milestone**: Milestone 2 Iteration 2 (Sandboxed GUI Automation & Visual Redaction)  
**Author**: `explorer_m2_it2_2`  
**Date**: 2026-10-11T23:00:00Z  
**Status**: COMPLETE / ACTIONABLE  

---

## 1. Executive Summary

During adversarial review of Milestone 2 (`reviewer_m2_2`), a major architectural defect (Finding 2) was identified in `aura-sdk/aura_sdk/tools/computer.py`:
When a connection error or EOF occurs during command execution in `AsyncComputerClient.send_command`, the underlying `_writer` and `_reader` stream handles remain non-`None`. Consequently, `is_connected` continues returning `True`, which prevents `_connect_locked()` from being invoked on subsequent calls. The client remains permanently incapacitated after any transient connection drop, throwing continuous `BrokenPipeError` or `ConnectionResetError` on every subsequent command.

This report provides the complete forensic diagnosis, architectural solution, and drop-in remediation artifacts (`proposed_computer.py`, `computer_remediation.patch`, `test_remediation.py`, `test_remediation.patch`) to make `AsyncComputerClient` fully resilient and self-healing.

---

## 2. Forensic Analysis & Root Cause

### 2.1 Code Citations (`aura-sdk/aura_sdk/tools/computer.py`)

In lines 249–297 of `aura-sdk/aura_sdk/tools/computer.py`:
```python
@property
def is_connected(self) -> bool:
    return self._writer is not None and not self._writer.is_closing()

async def _connect_locked(self) -> None:
    if self.is_connected:
        return
    self._reader, self._writer = await asyncio.wait_for(
        asyncio.open_connection(self.host, self.port), timeout=self.timeout
    )

async def send_command(self, method: str, params: dict[str, Any] | None = None) -> dict[str, Any]:
    async with self._lock:
        if not self.is_connected:
            await self._connect_locked()
        assert self._reader is not None
        assert self._writer is not None

        ...
        self._writer.write(raw_line.encode("utf-8"))
        await self._writer.drain()

        line_bytes = await asyncio.wait_for(
            self._reader.readline(), timeout=self.timeout
        )
        if not line_bytes:
            raise ConnectionResetError("Connection closed by computer daemon")

        resp = json.loads(line_bytes.decode("utf-8").strip())
        ...
```

### 2.2 The Defect Mechanics

1. **Defunct Handle Retention**:
   When the daemon disconnects, terminates, or times out, `self._reader.readline()` returns `b""` (EOF). Line 287 raises `ConnectionResetError("Connection closed by computer daemon")`. However, neither `self._writer` nor `self._reader` are closed or cleared.

2. **Reconnection Suppression**:
   In Python `asyncio`, `StreamWriter.is_closing()` returns `False` unless `writer.close()` was explicitly invoked on the client side. Because `self._writer` is non-None and `not self._writer.is_closing()`, `self.is_connected` evaluates to `True`. Furthermore, `self._reader.at_eof()` is never inspected.
   When the caller sends the next command, `if not self.is_connected:` evaluates to `False`. The client bypasses `_connect_locked()` and attempts to write to the dead writer, failing immediately and permanently.

3. **The Deadlock Trap with Naive `close()`**:
   A naive attempt to solve this by catching exceptions in `send_command` and calling `await self.close()` introduces a critical deadlock:
   ```python
   # DEADLOCK TRAP:
   async def send_command(...):
       async with self._lock:          # Lock acquired here
           ...
           except Exception:
               await self.close()      # self.close() also does: async with self._lock:
               raise
   ```
   In Python standard library `asyncio`, `asyncio.Lock` is **non-reentrant**. Calling `await self.close()` while already holding `self._lock` causes a permanent deadlock.

4. **Application Error vs. Network Error Distinction**:
   When the daemon returns a JSON-RPC application error (e.g. `{"error": {"code": -32601, "message": "Method not found"}}`), the TCP stream is healthy and synchronized. Tearing down the connection on application errors would add unnecessary connection latency and invalidate active sessions. Network exceptions and EOFs must be cleanly separated from application-level JSON-RPC errors.

---

## 3. Recommended Drop-In Fix Strategy

The remediation consists of four complementary improvements in `AsyncComputerClient`:

### Strategy Component 1: Robust `is_connected` Evaluation
Check both reader and writer existence, check `writer.is_closing()`, and check `reader.at_eof()`:
```python
@property
def is_connected(self) -> bool:
    if self._writer is None or self._reader is None:
        return False
    return not (self._writer.is_closing() or self._reader.at_eof())
```
If the server abruptly sends a FIN/RST packet, `self._reader.at_eof()` becomes `True`, causing `is_connected` to immediately reflect the dropped state.

### Strategy Component 2: Deadlock-Free Locked Cleanup Helper (`_close_locked`)
Provide an internal lock-guarded cleanup helper:
```python
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

async def close(self) -> None:
    async with self._lock:
        await self._close_locked()
```
This routine safely closes the writer, suppresses teardown exceptions, resets both `_writer` and `_reader` to `None`, and can be called from within `send_command` or `_connect_locked` without lock re-entrancy issues.

### Strategy Component 3: Pre-Connect Hygiene in `_connect_locked`
Ensure any prior/stale socket handles are cleaned up before establishing a new connection:
```python
async def _connect_locked(self) -> None:
    if self.is_connected:
        return
    await self._close_locked()
    self._reader, self._writer = await asyncio.wait_for(
        asyncio.open_connection(self.host, self.port), timeout=self.timeout
    )
```

### Strategy Component 4: Network I/O Boundary Exception Handling in `send_command`
Wrap the socket connect, write, drain, readline, and JSON decode inside a `try...except Exception:` block that calls `await self._close_locked()` before re-raising. Keep the JSON-RPC application error check outside the `try` block:
```python
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
```

---

## 4. Exact Unified Diff (`computer_remediation.patch`)

```diff
--- a/aura-sdk/aura_sdk/tools/computer.py
+++ b/aura-sdk/aura_sdk/tools/computer.py
@@ -248,45 +248,64 @@
 
     @property
     def is_connected(self) -> bool:
-        return self._writer is not None and not self._writer.is_closing()
+        if self._writer is None or self._reader is None:
+            return False
+        return not (self._writer.is_closing() or self._reader.at_eof())
 
     async def connect(self) -> None:
         async with self._lock:
             await self._connect_locked()
 
+    async def _close_locked(self) -> None:
+        """Closes stream reader and writer, resetting connection state while holding lock."""
+        if self._writer is not None:
+            try:
+                self._writer.close()
+                await self._writer.wait_closed()
+            except Exception:  # noqa: BLE001, S110
+                pass
+            self._writer = None
+        self._reader = None
+
     async def _connect_locked(self) -> None:
         if self.is_connected:
             return
+        await self._close_locked()
         self._reader, self._writer = await asyncio.wait_for(
             asyncio.open_connection(self.host, self.port), timeout=self.timeout
         )
 
     async def send_command(self, method: str, params: dict[str, Any] | None = None) -> dict[str, Any]:
         async with self._lock:
-            if not self.is_connected:
-                await self._connect_locked()
-            assert self._reader is not None
-            assert self._writer is not None
-
-            self._msg_id += 1
-            cmd_id = self._msg_id
-            payload = {
-                "jsonrpc": "2.0",
-                "id": cmd_id,
-                "method": method,
-                "params": params or {},
-            }
-            raw_line = json.dumps(payload) + "\n"
-            self._writer.write(raw_line.encode("utf-8"))
-            await self._writer.drain()
-
-            line_bytes = await asyncio.wait_for(
-                self._reader.readline(), timeout=self.timeout
-            )
-            if not line_bytes:
-                raise ConnectionResetError("Connection closed by computer daemon")
+            try:
+                if not self.is_connected:
+                    await self._connect_locked()
+                assert self._reader is not None
+                assert self._writer is not None
+
+                self._msg_id += 1
+                cmd_id = self._msg_id
+                payload = {
+                    "jsonrpc": "2.0",
+                    "id": cmd_id,
+                    "method": method,
+                    "params": params or {},
+                }
+                raw_line = json.dumps(payload) + "\n"
+                self._writer.write(raw_line.encode("utf-8"))
+                await self._writer.drain()
+
+                line_bytes = await asyncio.wait_for(
+                    self._reader.readline(), timeout=self.timeout
+                )
+                if not line_bytes:
+                    raise ConnectionResetError("Connection closed by computer daemon")
+
+                resp = json.loads(line_bytes.decode("utf-8").strip())
+            except Exception:
+                await self._close_locked()
+                raise
 
-            resp = json.loads(line_bytes.decode("utf-8").strip())
             if "error" in resp and resp["error"] is not None:
                 err = resp["error"]
                 msg = err.get("message", "Unknown error")
@@ -297,14 +316,7 @@
 
     async def close(self) -> None:
         async with self._lock:
-            if self._writer is not None:
-                try:
-                    self._writer.close()
-                    await self._writer.wait_closed()
-                except Exception:
-                    pass
-                self._writer = None
-                self._reader = None
+            await self._close_locked()
```

---

## 5. Empirical Verification & Test Evidence

The proposed implementation was thoroughly validated across three test suites:

1. **Remediation Test Suite (`test_remediation.py`)**:
   - `test_resilient_client_drop_and_reconnect`: Verifies mid-flight daemon drops reset handles to `None` and subsequent commands auto-reconnect successfully. (**PASS**)
   - `test_resilient_client_application_error_preserves_connection`: Verifies JSON-RPC `-32601` error does not disconnect healthy stream. (**PASS**)
   - `test_resilient_client_read_timeout_cleanup`: Verifies 100ms timeout cleans up handles and sets `is_connected=False`. (**PASS**)
   - `test_resilient_client_concurrency`: Verifies 10 parallel requests serialize without deadlock. (**PASS**)
   - `test_computer_tool_integration_with_resilient_client`: Verifies `ComputerTool` survives disconnect. (**PASS**)
   - Result: **5 passed, 0 failed in 5.41s**.

2. **Integration Test Suite (`proposed_test_additions.py`)**:
   - Tested against `proposed_computer.py`.
   - Result: **3 passed, 0 failed in 5.42s**.

3. **Regression Test Suite (`aura-sdk/tests/test_computer_tool.py`)**:
   - All 12 existing unit/integration tests (`test_computer_tool_*`, `test_mask_image_bytes_*`) pass cleanly against `proposed_computer.py`.
   - Result: **12 passed, 0 failed in 0.36s**.

4. **Lint & Code Style**:
   - `/home/jeff/aura/aura-sdk/.venv/bin/ruff check aura-sdk/aura_sdk aura-sdk/tests`: **All checks passed!**

---

## 6. Actionable Implementation Instructions for Worker (`worker_m2_1`)

1. Apply the patch `computer_remediation.patch` to `aura-sdk/aura_sdk/tools/computer.py` (or replace with `proposed_computer.py`).
2. Apply the patch `test_remediation.patch` to `aura-sdk/tests/test_computer_tool.py`.
3. Verify test passage:
   ```bash
   /home/jeff/aura/aura-sdk/.venv/bin/pytest aura-sdk/tests/test_computer_tool.py
   /home/jeff/aura/aura-sdk/.venv/bin/ruff check aura-sdk/aura_sdk/tools/computer.py aura-sdk/tests/test_computer_tool.py
   ```
4. Run full Python test suite:
   ```bash
   /home/jeff/aura/aura-sdk/.venv/bin/pytest aura-sdk/tests
   ```

---

## 7. Artifact Index

All generated drop-in artifacts are located in `/home/jeff/aura/.agents/teamwork/explorer_m2_it2_2/`:
- `proposed_computer.py`: Full drop-in source replacement for `aura-sdk/aura_sdk/tools/computer.py`.
- `computer_remediation.patch`: Exact unified diff patch for `aura-sdk/aura_sdk/tools/computer.py`.
- `proposed_test_additions.py`: New unit and integration test functions for `test_computer_tool.py`.
- `test_remediation.patch`: Exact unified diff patch adding resilience tests to `aura-sdk/tests/test_computer_tool.py`.
- `test_remediation.py`: Standalone verification test harness.
