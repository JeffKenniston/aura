# Forensic Test Suite Reconciliation Report: Milestone 4 (TOOL-BASH-002, TOOL-BRW-003)

**Author**: `explorer_m4_fix_3`  
**Working Directory**: `/home/jeff/aura/.agents/teamwork/explorer_m4_fix_3/`  
**Parent Conversation ID**: `ad091f9f-df8d-4332-bf82-a7111f5328d2`  
**Target**: Holistic Test Suite Reconciliation for `aura-sdk/tests/` (84 tests) and `aura-core/tests/`  
**Integrity Mode**: Demo Mode (per `ORIGINAL_REQUEST.md:84`)  

---

## Executive Summary

1. **Current Test Suite Baseline**:
   - `pytest --collect-only aura-sdk/tests` collects **84 tests** across 13 test modules.
   - Running `pytest -v aura-sdk/tests` currently results in **81 PASSED, 3 FAILED**:
     - `FAILED aura-sdk/tests/test_bash_stress.py::test_adversarial_space_in_working_directory`
     - `FAILED aura-sdk/tests/test_bash_stress.py::test_adversarial_unset_environment_variable`
     - `FAILED aura-sdk/tests/test_bash_stress.py::test_adversarial_stderr_capture_on_command_failure`
   - In `aura-core`, `cargo test -p aura-core --test m4_empirical_bash_session_stress_test` currently results in **5 PASSED, 3 FAILED** (same root causes in Rust host subshell & mutex serialization).

2. **Critical Discovery — Test Suite Reconciliation Dilemma**:
   - In `test_browser_tool_stress.py`, Challenger 2 authored 11 tests. 8 test normal behavior and 3 tests were authored as *proof-of-vulnerability* tests asserting bug occurrences:
     1. `test_screenshot_over_1mb_fails_due_to_websockets_default_max_size` (asserts `with pytest.raises(ConnectionClosedError)`)
     2. `test_cold_client_concurrent_connect_race_condition` (asserts `with pytest.raises(TimeoutError)`)
     3. `test_server_disconnect_leaves_stale_ws_handle` (asserts `with pytest.raises(ConnectionClosed)`)
   - **Crucial Finding**: When the production remediation proposed by `explorer_m4_fix_2` (`proposed_browser.py`: `max_size=32MB`, mutex-guarded `connect()`, and disconnect cleanup) is applied, **those 3 tests in `test_browser_tool_stress.py` fail** with `Failed: DID NOT RAISE ...` because the bugs no longer exist!
   - Therefore, a complete reconciliation requires updating these 3 test cases in `test_browser_tool_stress.py` to assert the resilient, production-grade behavior.

3. **Reconciliation Proof**:
   - When `proposed_bash.py` (from `explorer_m4_fix_1`) and `proposed_browser.py` (from `explorer_m4_fix_2`) are paired with the reconciled assertions in `test_browser_tool_stress.py`, **100% of the Python test suite passes: 84 passed, 0 failed in 3.55s**.

---

## 1. Observation

### 1.1 Complete Test Inventory in `aura-sdk/tests/` (84 Tests)

| Test Module | Test Name | Current Status | Cause |
|---|---|---|---|
| `test_bash_stress.py` | `test_rapid_sequential_env_mutations` | PASSED | Normal env mutations |
| `test_bash_stress.py` | `test_env_var_update_and_concatenation` | PASSED | Concatenation |
| `test_bash_stress.py` | `test_nested_working_directory_traversal` | PASSED | Standard cd traversal |
| `test_bash_stress.py` | `test_invalid_cd_does_not_break_working_directory` | PASSED | Error recovery |
| `test_bash_stress.py` | `test_multi_session_isolation_and_interleaving` | PASSED | Session map isolation |
| `test_bash_stress.py` | `test_state_continuity_after_non_zero_exit_codes` | PASSED | State preserved |
| `test_bash_stress.py` | `test_syntax_error_resilience` | PASSED | Syntax error recovery |
| `test_bash_stress.py` | `test_reset_and_reinitialize_session` | PASSED | Reset method |
| `test_bash_stress.py` | `test_high_throughput_sequential_pipeline` | PASSED | 30 sequential calls |
| `test_bash_stress.py` | `test_adversarial_space_in_working_directory` | **FAILED** | Unquoted `cd {cwd}` in `bash.py:71` |
| `test_bash_stress.py` | `test_adversarial_unset_environment_variable` | **FAILED** | Additive mutation of `session["env"]` in `bash.py:106` |
| `test_bash_stress.py` | `test_adversarial_stderr_capture_on_command_failure` | **FAILED** | Uncaptured exit code; `env -0` masks returncode to 0 in `bash.py:77,109` |
| `test_bash_tool.py` | 7 baseline unit tests | 7 PASSED | All baseline tests pass |
| `test_browser_tool.py` | 6 baseline unit tests | 6 PASSED | All baseline tests pass |
| `test_browser_tool_stress.py` | 8 baseline stress tests | 8 PASSED | Unsolicited events, fallbacks, complex eval, etc. |
| `test_browser_tool_stress.py` | `test_screenshot_over_1mb_fails_due_to_websockets_default_max_size` | PASSED (Proof) | Asserts `ConnectionClosedError` occurs |
| `test_browser_tool_stress.py` | `test_cold_client_concurrent_connect_race_condition` | PASSED (Proof) | Asserts `TimeoutError` occurs |
| `test_browser_tool_stress.py` | `test_server_disconnect_leaves_stale_ws_handle` | PASSED (Proof) | Asserts `ConnectionClosed` occurs |
| `test_bus_client.py` | 5 bus integration tests | 5 PASSED | Zenoh message envelope & SVID |
| `test_cognition_gemini.py` | 11 cognition tests | 11 PASSED | Gemini interactions, file search, tools |
| `test_cognitive_cache.py` | 1 compression test | 1 PASSED | Cognitive cache compression |
| `test_cognitive_loop.py` | 1 loop test | 1 PASSED | Read-switch-execute-return |
| `test_context_saturation.py` | 5 saturation tests | 5 PASSED | Model limits & Flash-Lite summarization |
| `test_router.py` | 13 router tier tests | 13 PASSED | Decision heuristics, Pro blocking, Flash |
| `test_router_cache_context.py` | 6 router cache tests | 6 PASSED | Semantic cache routing |
| `test_semantic_cache.py` | 5 cache tests | 5 PASSED | BLAKE3 hashing & Zenoh cache store |
| `test_transport.py` | 1 transport test | 1 PASSED | Zenoh client connect |

**Current Run Result**: `81 passed, 3 failed, 1 warning in 7.05s`.

---

### 1.2 Verbatim Failure Observations in `aura-sdk/tests/test_bash_stress.py`

#### Failure 1: Unquoted Path With Spaces (`test_adversarial_space_in_working_directory`)
```text
_________________ test_adversarial_space_in_working_directory __________________
    await tool.execute(f'cd "{temp_root}"', session_id=sid)
    pwd_out = await tool.execute("pwd", session_id=sid)
>   assert os.path.realpath(pwd_out) == os.path.realpath(temp_root)
E   AssertionError: Expected cwd to persist as /tmp/aura space test g0k621hx, but got /home/jeff/aura
E   assert '/home/jeff/aura' == '/tmp/aura sp...test g0k621hx'
```
- **Line Citation**: `aura-sdk/aura_sdk/tools/bash.py:71`:
  ```python
  script = (
      f"cd {session['cwd']} 2>/dev/null || true\n"
      f"{command}\n"
  ...
  ```
  `{session['cwd']}` is unquoted. Bash receives multiple arguments `cd /tmp/aura space test ...`, fails with `bash: cd: too many arguments`, and stays in the previous directory.

#### Failure 2: Inability to Unset Environment Variables (`test_adversarial_unset_environment_variable`)
```text
_________________ test_adversarial_unset_environment_variable __________________
    await tool.execute("export UNSET_ME='present'", session_id=sid)
    await tool.execute("unset UNSET_ME", session_id=sid)
    out_after = await tool.execute("echo \"[${UNSET_ME}]\"", session_id=sid)
>   assert out_after == "[]", f"Expected UNSET_ME to be cleared, but got {out_after}"
E   AssertionError: Expected UNSET_ME to be cleared, but got [present]
E   assert '[present]' == '[]'
```
- **Line Citation**: `aura-sdk/aura_sdk/tools/bash.py:103-106`:
  ```python
  for item in env_part.split(b"\x00"):
      if b"=" in item:
          k, v = item.split(b"=", 1)
          session["env"][k.decode("utf-8", errors="replace")] = v.decode("utf-8", errors="replace")
  ```
  `session["env"]` is updated additively. When `unset UNSET_ME` is called, `UNSET_ME` is omitted from `env -0`, but the key remains in `session["env"]` and is re-injected on the next invocation via `proc = await asyncio.create_subprocess_shell(..., env=session["env"])`.

#### Failure 3: Silent Stderr Swallowing on Command Failure (`test_adversarial_stderr_capture_on_command_failure`)
```text
______________ test_adversarial_stderr_capture_on_command_failure ______________
    err_out = await tool.execute("ls /nonexistent_aura_file_should_fail_12345", session_id=sid)
>   assert err_out != "", "Expected stderr output on failure, but got empty string"
E   AssertionError: Expected stderr output on failure, but got empty string
E   assert '' != ''
```
- **Line Citation**: `aura-sdk/aura_sdk/tools/bash.py:70-78, 108-111`:
  ```python
  script = (
      f"cd {session['cwd']} 2>/dev/null || true\n"
      f"{command}\n"
      f"__AURA_EXIT__=$?\n"
      f"echo -n '__AURA_CWD_MARKER__'\n"
      f"pwd\n"
      f"echo -n '__AURA_ENV_MARKER__'\n"
      f"env -0\n"
  )
  ...
  output_str = user_stdout.decode("utf-8", errors="replace").rstrip("\n")
  if proc.returncode != 0 and stderr_bytes and not output_str:
      return stderr_bytes.decode("utf-8", errors="replace").rstrip()
  return output_str
  ```
  `__AURA_EXIT__` is assigned to a shell variable but never printed. Because `env -0` executes last, `proc.returncode` is always 0. The error handling branch `if proc.returncode != 0` is dead code. When a command produces stderr and no stdout, `user_stdout` is empty and `""` is returned.

---

### 1.3 Verbatim Failure Observations When Remediating `browser.py` Without Test Reconciliation

When `proposed_browser.py` from `explorer_m4_fix_2` was tested against un-reconciled `aura-sdk/tests/test_browser_tool_stress.py`, the following 3 tests failed:

```text
______ test_screenshot_over_1mb_fails_due_to_websockets_default_max_size _______
    tool = BrowserTool(cdp_url=stress_cdp_server["cdp_url"])
    stress_cdp_server["control"]["screenshot_size"] = 1_500_000
>   with pytest.raises(websockets.exceptions.ConnectionClosedError) as exc_info:
        await tool.screenshot(format="png")
E   Failed: DID NOT RAISE ConnectionClosedError

______________ test_cold_client_concurrent_connect_race_condition ______________
    tasks = [call_worker(i) for i in range(5)]
>   with pytest.raises((TimeoutError, asyncio.TimeoutError)):
        await asyncio.wait_for(asyncio.gather(*tasks), timeout=3.0)
E   Failed: DID NOT RAISE any of (TimeoutError, TimeoutError)

________________ test_server_disconnect_leaves_stale_ws_handle _________________
    stress_cdp_server["control"]["force_disconnect"] = False
>   with pytest.raises(websockets.exceptions.ConnectionClosed):
        await tool.evaluate("after_disconnect_unhandled")
E   Failed: DID NOT RAISE ConnectionClosed
```

---

## 2. Logic Chain

1. **Root Cause Analysis of Failing Tests**:
   - The 3 failures in `test_bash_stress.py` are genuine bugs in `bash.py`:
     - Space in path: unquoted `{session['cwd']}` fails bash `cd`.
     - Unset env: additive dictionary mutation fails to prune deleted keys.
     - Stderr capture: pipeline trailing `env -0` clobbers process exit code.
   - These 3 bugs must be remedied in `bash.py` using quoted cwd, fresh environment replacement, and exit code marker emission.

2. **Verification of `proposed_bash.py`**:
   - `explorer_m4_fix_1` authored `proposed_bash.py`.
   - Running the adversarial test cases against `proposed_bash.py` confirmed:
     - Directory paths with spaces persist correctly.
     - Unset variables are purged from subsequent invocations.
     - Stderr is captured and returned when exit code != 0 and stdout is empty.
   - All 12/12 tests in `test_bash_stress.py` pass.

3. **Reconciliation Necessity in `test_browser_tool_stress.py`**:
   - In `test_browser_tool_stress.py`, tests 5, 7, and 11 were written by `challenger_m4_2` to empirically demonstrate that bugs BUG-BRW-001, BUG-BRW-002, and BUG-BRW-003 existed in `browser.py`.
   - Once `browser.py` is fixed to handle >1MB screenshots, serialize cold-client connections, and clean up dropped WebSocket connections:
     - 1.5MB screenshots succeed without raising `ConnectionClosedError(1009)`.
     - 5 concurrent cold requests succeed without raising `TimeoutError`.
     - Commands after server reconnect succeed without raising `ConnectionClosed`.
   - Tests expecting these operations to fail will inevitably fail once the system is fixed.
   - Therefore, the test suite must be reconciled to assert the positive, hardened system behavior.

4. **Empirical Proof of 100% Pass Rate**:
   - When `proposed_bash.py`, `proposed_browser.py`, and the reconciled `test_browser_tool_stress.py` were executed together across the entire test suite:
   - Output: `84 passed, 1 warning in 3.55s (exit code 0)`.
   - Zero failures, zero skips, zero errors.

---

## 3. Caveats

1. **Rust Core Tests in `aura-core`**:
   - In `aura-core/tests/m4_empirical_bash_session_stress_test.rs`, 3 tests fail for identical reasons in Rust:
     - `test_bash_session_adversarial_cwd_with_spaces` (unquoted cwd in `firecracker.rs:929`)
     - `test_bash_session_adversarial_unset_env` (`self.current_env.insert` without clear in `firecracker.rs:966`)
     - `test_bash_session_cross_session_blocking_lock` (global `sessions.lock().await` held across `session.execute().await` in `firecracker.rs:1032`)
   - The worker applying the remediation must patch `firecracker.rs` as detailed in Explorer M4 Fix 1's report to achieve 146/146 tests passing in `aura-core`.
2. **Read-Only Explorer Constraint**:
   - Explorer M4 Fix 3 performed empirical tests in memory and via monkeypatching without mutating files outside `.agents/teamwork/explorer_m4_fix_3/`.

---

## 4. Conclusion & Concrete Actionable Remediation Plan

### Remediation Step 1: Update `aura-sdk/aura_sdk/tools/bash.py`
Replace lines 70–114 with:
```python
        escaped_cwd = session["cwd"].replace('"', '\\"')
        script = (
            f'cd "{escaped_cwd}" 2>/dev/null || true\n'
            f"{command}\n"
            f"__AURA_STATUS__=$?\n"
            f"echo -n '__AURA_EXIT_MARKER__'\n"
            f'echo "$__AURA_STATUS__"\n'
            f"echo -n '__AURA_CWD_MARKER__'\n"
            f"pwd\n"
            f"echo -n '__AURA_ENV_MARKER__'\n"
            f"env -0\n"
        )

        proc = await asyncio.create_subprocess_shell(
            script,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
            env=session["env"],
        )

        stdout_bytes, stderr_bytes = await asyncio.wait_for(
            proc.communicate(),
            timeout=self.timeout,
        )

        if b"__AURA_EXIT_MARKER__" in stdout_bytes:
            parts = stdout_bytes.split(b"__AURA_EXIT_MARKER__", 1)
            user_stdout = parts[0]
            rest = parts[1]

            exit_code = proc.returncode
            if b"__AURA_CWD_MARKER__" in rest:
                exit_part, rest = rest.split(b"__AURA_CWD_MARKER__", 1)
                try:
                    exit_code = int(exit_part.decode("utf-8", errors="replace").strip())
                except ValueError:
                    pass

                if b"__AURA_ENV_MARKER__" in rest:
                    cwd_part, env_part = rest.split(b"__AURA_ENV_MARKER__", 1)
                    new_cwd = cwd_part.decode("utf-8", errors="replace").strip()
                    if os.path.exists(new_cwd):
                        session["cwd"] = new_cwd

                    new_env = {}
                    for item in env_part.split(b"\x00"):
                        if b"=" in item:
                            k, v = item.split(b"=", 1)
                            new_env[k.decode("utf-8", errors="replace")] = v.decode("utf-8", errors="replace")
                    session["env"] = new_env

            output_str = user_stdout.decode("utf-8", errors="replace").rstrip("\n")
            if exit_code != 0 and stderr_bytes and not output_str:
                return stderr_bytes.decode("utf-8", errors="replace").rstrip()
            return output_str

        output_str = stdout_bytes.decode("utf-8", errors="replace").rstrip("\n")
        if proc.returncode != 0 and stderr_bytes and not output_str:
            return stderr_bytes.decode("utf-8", errors="replace").rstrip()
        return output_str
```

### Remediation Step 2: Update `aura-sdk/aura_sdk/tools/browser.py`
Use `proposed_browser.py` from `explorer_m4_fix_2`:
1. `max_size=32 * 1024 * 1024` on `websockets.connect`
2. `self._connect_locked()` inside `async with self._lock:`
3. Reset `self._ws = None` on `ConnectionClosed` / `BrokenPipeError` in `send_command`

### Remediation Step 3: Reconcile `aura-sdk/tests/test_browser_tool_stress.py`
Replace the 3 proof-of-bug tests with resilient assertions:

```python
# 1. Replace lines 312-329:
@pytest.mark.asyncio
async def test_screenshot_over_1mb_succeeds(stress_cdp_server):
    """
    Verifies that screenshots exceeding 1MB (e.g. 1.5MB) stream successfully
    without triggering websockets default 1009 (MESSAGE_TOO_BIG) error.
    """
    tool = BrowserTool(cdp_url=stress_cdp_server["cdp_url"])
    # 1.5 MB screenshot
    stress_cdp_server["control"]["screenshot_size"] = 1_500_000

    png_bytes = await tool.screenshot(format="png")
    assert isinstance(png_bytes, bytes)
    assert png_bytes.startswith(PNG_MAGIC_BYTES)
    assert len(png_bytes) == 1_500_000
    await tool.close()

# 2. Replace lines 350-368:
@pytest.mark.asyncio
async def test_cold_client_concurrent_connect_synchronized(stress_cdp_server):
    """
    Verifies that concurrent calls on a cold client synchronize connection
    safely without dropped responses or TimeoutError.
    """
    tool = BrowserTool(cdp_url=stress_cdp_server["cdp_url"])

    async def call_worker(idx: int):
        return await tool.evaluate(f"cold_{idx}")

    tasks = [call_worker(i) for i in range(5)]
    results = await asyncio.wait_for(asyncio.gather(*tasks), timeout=3.0)
    for i in range(5):
        assert f"eval_echo:cold_{i}" in results

    await tool.close()

# 3. Replace lines 421-449:
@pytest.mark.asyncio
async def test_server_disconnect_auto_recovery(stress_cdp_server):
    """
    Verifies that when the server drops connection mid-flight,
    the active command raises ConnectionClosed, and subsequent commands
    automatically re-establish connection and execute cleanly.
    """
    tool = BrowserTool(cdp_url=stress_cdp_server["cdp_url"])
    res = await tool.evaluate("before_disconnect")
    assert res == "eval_echo:before_disconnect"

    # Force server disconnect
    stress_cdp_server["control"]["force_disconnect"] = True
    with pytest.raises(websockets.exceptions.ConnectionClosed):
        await tool.evaluate("trigger_disconnect")

    # Server is back up
    stress_cdp_server["control"]["force_disconnect"] = False

    # Automatically reconnects and executes
    recovered = await tool.evaluate("after_disconnect_recovered")
    assert recovered == "eval_echo:after_disconnect_recovered"
    await tool.close()
```

### Remediation Step 4: Update `aura-core/src/hypervisor/firecracker.rs`
1. Quote cwd: line 929: `cd "{}" 2>/dev/null || true`
2. Clear env map before populating: line 960: `self.current_env.clear();`
3. Per-session lock in `BashSessionManager`:
   Store `Arc<tokio::sync::Mutex<BashSessionHandle>>` in `HashMap` so `session.execute()` is awaited outside the manager-level map lock.

---

## 5. Verification Method

### 5.1 Python SDK Test Suite Verification
```bash
cd /home/jeff/aura
source aura-sdk/.venv/bin/activate
pytest -v aura-sdk/tests
```
- **Expected Result**: **84 passed, 0 failed, 1 warning** across all 14 test modules in under 4 seconds.

### 5.2 Isolated Stress Tests Verification
```bash
# Bash tool stress tests
pytest -v aura-sdk/tests/test_bash_stress.py
# Expected Result: 12 passed, 0 failed

# Browser tool stress tests
pytest -v aura-sdk/tests/test_browser_tool_stress.py
# Expected Result: 11 passed, 0 failed
```

### 5.3 Rust Hypervisor & Stress Test Suite Verification
```bash
cargo check --all-targets
cargo test -p aura-core
```
- **Expected Result**: **146 passed, 0 failed** across all unit and integration tests (including `m4_empirical_bash_session_stress_test`, `m4_empirical_cdp_forwarder_stress_test`, `http3_stream_concurrency_test`, and `m4_empirical_git_stress_test`).

### 5.4 Invalidation Conditions
This reconciliation report is invalidated if:
1. Any test in `aura-sdk/tests/` fails after applying the 3 remediation patches.
2. Less than 84 tests are collected and run in `aura-sdk/tests/`.
3. Any mock, stub, or TODO is introduced into production code.
