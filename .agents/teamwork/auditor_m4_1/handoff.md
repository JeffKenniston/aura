# Forensic Audit & Handoff Report: Milestone 4 (TOOL-BASH-002, TOOL-BRW-003, GAP-003)

**Auditor ID**: `auditor_m4_1`  
**Working Directory**: `/home/jeff/aura/.agents/teamwork/auditor_m4_1/`  
**Parent Conversation ID**: `ad091f9f-df8d-4332-bf82-a7111f5328d2`  
**Target**: Milestone 4: Persistent Stateful Tools & GAP-003  
**Integrity Mode**: Demo (per `ORIGINAL_REQUEST.md:84` & `DISPATCH.md:6`)  
**Verdict**: **INTEGRITY VIOLATION**  

---

## Forensic Audit Report Summary

```markdown
## Forensic Audit Report

**Work Product**: Milestone 4 Deliverables:
  - `deploy/images/vsock-shell-daemon/`
  - `aura-core/src/hypervisor/firecracker.rs`
  - `aura-core/src/hypervisor/wasm.rs`
  - `docs/architecture/adrs/ADR-001-Execution-Host.md`
  - `.agents/plugins/aura-dev-kit/skills/aura-architecture/references/02-execution-sandboxes.md`
  - `aura-sdk/aura_sdk/tools/bash.py`
  - `aura-sdk/aura_sdk/tools/browser.py`
  - `aura-sdk/tests/test_bash_tool.py`
  - `aura-sdk/tests/test_browser_tool.py`
**Profile**: General Project (Demo Mode)
**Verdict**: INTEGRITY VIOLATION

### Phase Results
- [Hardcoded test results detection]: PASS — No hardcoded test result returns in production paths.
- [Facade detection check]: FAIL — `BashTool` in `bash.py` implements an unreachable dead-code error handling branch (`if proc.returncode != 0 and stderr_bytes:`) because `env -0` unconditionally executes last in the pipeline, masking actual exit codes and silently swallowing stderr output.
- [Pre-populated artifact detection]: PASS — No pre-populated logs or fabricated attestation artifacts exist in the workspace.
- [Zero mock/stub/TODO constitutional check (AGENTS.md)]: PASS — Grep confirms 0 occurrences of `TODO`, `FIXME`, `STUB`, or `unimplemented!` across all production sources.
- [Authentic TOOL-BASH-002 Daemon & Microkernel]: PASS — Standalone in-guest Rust shell daemon and microkernel `BashSessionManager` / `BashSessionHandle` are genuine, compiling cleanly with 8 daemon tests and 20 hypervisor tests passing.
- [Authentic TOOL-BRW-003 Forwarder]: PASS — `CdpVsockForwarder` in `firecracker.rs` implements genuine TCP-to-vsock UDS loopback proxying with byte-by-byte handshake parsing.
- [Formal GAP-003 Architecture Documentation]: PASS — Formally documented in `wasm.rs:1-18`, `ADR-001:19-28` (Decision 5), and `02-execution-sandboxes.md:19`.
- [Independent compiler verification]: PASS — `cargo check --all-targets` completed cleanly in 7.49s (0 warnings, 0 errors).
- [Independent Rust core test execution]: PASS — `cargo test -p aura-core` passed 138/138 tests (0 failures, 0 ignored).
- [Independent daemon test execution]: PASS — `cargo test --manifest-path deploy/images/vsock-shell-daemon/Cargo.toml` passed 8/8 tests.
- [Independent Python SDK test suite execution]: FAIL — `source aura-sdk/.venv/bin/activate && pytest aura-sdk/tests` FAILED with 6 test failures (exit code 1).
```

---

## 1. Observation

### 1.1 Verbatim Tool Execution Outputs

#### Check 1: Workspace Target Compilation (`cargo check --all-targets`)
```text
$ cargo check --all-targets
Checking aura-core v0.1.0 (/home/jeff/aura/aura-core)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.49s
(Exit code: 0)
```

#### Check 2: Microkernel Core Tests (`cargo test -p aura-core`)
```text
$ cargo test -p aura-core
test result: ok. 126 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.73s
Running tests/http3_stream_concurrency_test.rs: 2 passed; 0 failed; finished in 1.90s
Running tests/m4_empirical_cdp_forwarder_stress_test.rs: 5 passed; 0 failed; finished in 0.07s
Running tests/m4_empirical_git_stress_test.rs: 5 passed; 0 failed; finished in 0.08s
Total: 138 passed, 0 failed.
(Exit code: 0)
```

#### Check 3: In-Guest Shell Daemon Tests (`cargo test --manifest-path deploy/images/vsock-shell-daemon/Cargo.toml`)
```text
$ cargo test --manifest-path deploy/images/vsock-shell-daemon/Cargo.toml
running 8 tests
test server::tests::test_handle_client_json_roundtrip ... ok
test session::tests::test_session_manager_close_and_ping ... ok
test session::tests::test_shell_session_basic_execution ... ok
test session::tests::test_shell_session_environment_variable_continuity ... ok
test session::tests::test_shell_session_working_directory_continuity ... ok
test server::tests::test_tcp_listener_ipc_e2e ... ok
test session::tests::test_session_manager_isolation ... ok
test session::tests::test_shell_session_non_zero_exit_code ... ok
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
(Exit code: 0)
```

#### Check 4: Workspace Python Test Suite (`source aura-sdk/.venv/bin/activate && pytest aura-sdk/tests`)
```text
$ source aura-sdk/.venv/bin/activate && pytest aura-sdk/tests
=========================== short test summary info ============================
FAILED aura-sdk/tests/test_bash_stress.py::test_adversarial_space_in_working_directory
FAILED aura-sdk/tests/test_bash_stress.py::test_adversarial_unset_environment_variable
FAILED aura-sdk/tests/test_bash_stress.py::test_adversarial_stderr_capture_on_command_failure
FAILED aura-sdk/tests/test_browser_tool_stress.py::test_invalid_selectors_syntax_and_missing
FAILED aura-sdk/tests/test_browser_tool_stress.py::test_large_screenshot_streaming
FAILED aura-sdk/tests/test_browser_tool_stress.py::test_high_concurrency_serialization
=================== 6 failed, 67 passed, 1 warning in 13.67s ===================
(Exit code: 1)
```

#### Check 5: Isolated Baseline Tests (`pytest aura-sdk/tests --ignore=...`)
```text
$ pytest aura-sdk/tests --ignore=aura-sdk/tests/test_bash_stress.py --ignore=aura-sdk/tests/test_browser_tool_stress.py
======================== 61 passed, 1 warning in 3.19s =========================
(Exit code: 0)
```

#### Check 6: Constitutional grep for Prohibited Patterns (AGENTS.md)
Searches across `deploy/images/vsock-shell-daemon/src/`, `aura-core/src/`, and `aura-sdk/aura_sdk/tools/`:
- `TODO`: 0 matches in production code.
- `todo!`: 0 matches in production code.
- `unimplemented!`: 0 matches in production code.
- `FIXME`: 0 matches in production code.
- `stub`: 0 matches in production code.

---

### 1.2 Direct Code Observations of Identified Defects

#### Observation 1: Exit Code Masking & Silent Stderr Suppression in `aura-sdk/aura_sdk/tools/bash.py`
In `aura-sdk/aura_sdk/tools/bash.py:70-111`:
```python
70:         script = (
71:             f"cd {session['cwd']} 2>/dev/null || true\n"
72:             f"{command}\n"
73:             f"__AURA_EXIT__=$?\n"
74:             f"echo -n '__AURA_CWD_MARKER__'\n"
75:             f"pwd\n"
76:             f"echo -n '__AURA_ENV_MARKER__'\n"
77:             f"env -0\n"
78:         )
...
108:         output_str = user_stdout.decode("utf-8", errors="replace").rstrip("\n")
109:         if proc.returncode != 0 and stderr_bytes and not output_str:
110:             return stderr_bytes.decode("utf-8", errors="replace").rstrip()
111:         return output_str
```
- **Line 73** assigns the exit code of `command` to shell variable `__AURA_EXIT__`.
- However, lines 74–77 subsequently execute `echo`, `pwd`, and `env -0`.
- Because `env -0` is the final command in the pipeline, the subprocess returncode (`proc.returncode`) is **always 0**.
- The script never outputs `__AURA_EXIT__`, and the Python code never parses it.
- As a consequence, the condition on **line 109** (`if proc.returncode != 0`) is **dead code that evaluates to `False` on every command failure**.
- When a command fails and prints only to stderr (e.g. `ls /nonexistent_file`), `user_stdout` is empty and line 111 returns `""` (empty string), completely swallowing the error.
- Direct failure proof: `aura-sdk/tests/test_bash_stress.py::test_adversarial_stderr_capture_on_command_failure` fails:
  ```text
  AssertionError: Expected stderr output on failure, but got empty string
  assert '' != ''
  ```

#### Observation 2: Working Directory Truncation on Paths with Spaces in `aura-sdk/aura_sdk/tools/bash.py`
In `aura-sdk/aura_sdk/tools/bash.py:71`:
```python
71:             f"cd {session['cwd']} 2>/dev/null || true\n"
```
- `{session['cwd']}` is unquoted in the script template.
- When `session['cwd']` contains spaces (e.g. `/tmp/aura space test lk0grgaq`), bash evaluates `cd /tmp/aura space test lk0grgaq`, failing with `bash: cd: too many arguments`.
- Direct failure proof: `aura-sdk/tests/test_bash_stress.py::test_adversarial_space_in_working_directory` fails:
  ```text
  AssertionError: Expected cwd to persist as /tmp/aura space test lk0grgaq, but got /home/jeff/aura
  ```

#### Observation 3: Inability to Prune Unset Environment Variables in `aura-sdk/aura_sdk/tools/bash.py`
In `aura-sdk/aura_sdk/tools/bash.py:104-106`:
```python
104:                 for item in env_part.split(b"\x00"):
105:                     if b"=" in item:
106:                         session["env"][k.decode("utf-8", errors="replace")] = v.decode("utf-8", errors="replace")
```
- The session environment dictionary `session["env"]` is updated additively by setting keys.
- If a user command runs `unset MY_VAR`, `MY_VAR` is absent from `env -0`, but the loop on line 104 does not purge keys missing from the new environment snapshot.
- Direct failure proof: `aura-sdk/tests/test_bash_stress.py::test_adversarial_unset_environment_variable` fails:
  ```text
  AssertionError: Expected UNSET_ME to be cleared, but got [present]
  assert '[present]' == '[]'
  ```

#### Observation 4: Frame Truncation on Screenshots > 1MB in `aura-sdk/aura_sdk/tools/browser.py`
In `aura-sdk/aura_sdk/tools/browser.py:76`:
```python
76:         self._ws = await websockets.connect(self._ws_url)
```
- `websockets.connect` defaults to `max_size = 1048576` (1 MiB).
- Modern headless Chromium full-page or high-DPI screenshots regularly exceed 1 MiB in raw PNG base64 size.
- When payload exceeds 1 MiB, the client triggers an unhandled `websockets.exceptions.ConnectionClosedError`:
  ```text
  websockets.exceptions.ConnectionClosedError: sent 1009 (message too big) frame exceeds limit of 1048576 bytes; no close frame received
  ```
- Direct failure proof: `aura-sdk/tests/test_browser_tool_stress.py::test_large_screenshot_streaming` fails with `ConnectionClosedError`.

#### Observation 5: Race Condition on Connection Handshake in `aura-sdk/aura_sdk/tools/browser.py`
In `aura-sdk/aura_sdk/tools/browser.py:82-85`:
```python
82:         if self._ws is None:
83:             await self.connect()
84: 
85:         async with self._lock:
```
- `if self._ws is None: await self.connect()` is located outside `async with self._lock:`.
- When multiple coroutines concurrently call `evaluate()` or `navigate()`, multiple tasks simultaneously execute `self.connect()`, launching competing HTTP discovery requests and WebSocket connections that clobber `self._ws`.
- Direct failure proof: `aura-sdk/tests/test_browser_tool_stress.py::test_high_concurrency_serialization` times out and crashes.

---

## 2. Logic Chain

1. **Constitutional & Mandate Foundation**:
   - The Forensic Auditor operating manual states:
     - *"Trust nothing: Even if tests pass, the binary could be cheating."*
     - *"Verify empirically: Run every check yourself. Do not accept claims."*
     - *"Phase 2, Check 4 (Build and run): The build must succeed and tests must execute — a project that doesn't build or whose tests don't run is automatically flagged."*
     - *"Block on failure: If ANY check fails, the verdict is INTEGRITY VIOLATION and the work product must be rejected."*
     - *"Do not silently correct errors — they may indicate deeper problems."*

2. **Empirical Verification of Workspace Test Suite**:
   - The test command `source aura-sdk/.venv/bin/activate && pytest aura-sdk/tests` is an explicit acceptance verification gate.
   - Independent execution of this command returned exit code 1 with 6 test failures.
   - A work product with failing test suites in its active workspace cannot be certified as `CLEAN`.

3. **Facade & Dead-Code Analysis in `BashTool`**:
   - A core requirement of `TOOL-BASH-002` is reliable execution of bash commands in persistent sessions.
   - In `bash.py`, line 109 claims to handle errors: `if proc.returncode != 0 and stderr_bytes: return stderr`.
   - However, because the script places `env -0` at the very end of the script string without capturing and echoing `__AURA_EXIT__`, `proc.returncode` is always 0.
   - This creates a facade where the code appears to handle errors, but in practice swallows all command error outputs, returning empty strings on failures.

4. **Reliability Analysis in `BrowserTool`**:
   - `TOOL-BRW-003` requires capturing page screenshots and handling concurrent browser actions.
   - Leaving `websockets.connect` with the default 1MB frame limit guarantees that real-world browser screenshots fail with protocol error 1009.
   - Placing `self.connect()` outside `self._lock` induces concurrency race conditions.

5. **Verdict Inevitability**:
   - Because Check 4 fails (`pytest aura-sdk/tests` returns exit code 1 with 6 failures) and Check 2 fails (`BashTool` contains a dead-code error handling facade), the only valid verdict under the Forensic Auditor prompt is **INTEGRITY VIOLATION**.

---

## 3. Caveats

1. **Rust Microkernel & Daemon Excellence**:
   - The Rust components (`deploy/images/vsock-shell-daemon` and `aura-core/src/hypervisor/firecracker.rs`) are fully authentic, genuine, and exemplary.
   - All 138 `aura-core` tests and all 8 `vsock-shell-daemon` tests pass with zero failures.
   - In `vsock-shell-daemon/src/session.rs`, exit codes and stderr are properly captured and parsed; the issue is strictly isolated to the Python SDK implementations in `aura-sdk/aura_sdk/tools/bash.py` and `browser.py`.

2. **Baseline vs. Stress Test Suite Origin**:
   - Worker `worker_m4_3` originally delivered 61 unit tests (`test_bash_tool.py`, `test_browser_tool.py`, and preexisting tests), all of which pass cleanly.
   - The failing tests (`test_bash_stress.py` and `test_browser_tool_stress.py`) were introduced by the adversarial challenger agents (`challenger_m4_1`, `challenger_m4_2`).
   - However, because these tests reside in the active test directory `aura-sdk/tests/`, they are part of the empirical test suite and expose genuine behavioral bugs.

3. **Test Emulator Mismatch on One Test**:
   - In `test_browser_tool_stress.py::test_invalid_selectors_syntax_and_missing`, the failure is due to the test fixture emulator expecting string literal `"invalid_selector"` rather than the actual CSS selector `">>>"`. The remaining 5 failures reflect genuine flaws in production code.

---

## 4. Conclusion

### Final Assessment: **INTEGRITY VIOLATION**

The work product for Milestone 4 cannot be certified clean in its current state due to test suite failures and behavioral defects in `aura-sdk/aura_sdk/tools/bash.py` and `browser.py`.

### Required Actions for Remediation Worker:
1. **Fix `bash.py`**:
   - Quote working directory: `cd "{session['cwd']}" 2>/dev/null || true`
   - Capture and echo exit code:
     ```bash
     {command}
     __AURA_STATUS__=$?
     echo -n '__AURA_EXIT_MARKER__'
     echo "$__AURA_STATUS__"
     echo -n '__AURA_CWD_MARKER__'
     pwd
     echo -n '__AURA_ENV_MARKER__'
     env -0
     ```
   - Parse `__AURA_EXIT_MARKER__` in Python and use the parsed exit code rather than `proc.returncode`.
   - Update `session["env"]` by replacing it with the newly parsed environment mapping rather than additively mutating it, allowing `unset` to take effect.
   - Return stderr when exit code is non-zero and user stdout is empty.

2. **Fix `browser.py`**:
   - In `websockets.connect(self._ws_url, max_size=None)`: disable or increase the maximum frame size to at least 16 MiB (`max_size=16 * 1024 * 1024`) to support large screenshots.
   - Guard `self.connect()` with `async with self._lock:` or an initialization lock to prevent concurrent connection race conditions.

3. **Verify Remediation**:
   - Run `source aura-sdk/.venv/bin/activate && pytest aura-sdk/tests` and confirm that all 73+ tests pass with 0 failures.

---

## 5. Verification Method

To independently reproduce and verify this finding:

1. **Run full Python test suite**:
   ```bash
   cd /home/jeff/aura
   source aura-sdk/.venv/bin/activate
   pytest aura-sdk/tests
   ```
   *Expected Current Output*: Fails with `6 failed, 67 passed`.

2. **Run isolated bash stress tests**:
   ```bash
   source aura-sdk/.venv/bin/activate
   pytest aura-sdk/tests/test_bash_stress.py -v
   ```
   *Expected Current Output*: Fails on `test_adversarial_space_in_working_directory`, `test_adversarial_unset_environment_variable`, `test_adversarial_stderr_capture_on_command_failure`.

3. **Run isolated browser stress tests**:
   ```bash
   source aura-sdk/.venv/bin/activate
   pytest aura-sdk/tests/test_browser_tool_stress.py -k "test_large_screenshot_streaming or test_high_concurrency_serialization" -v
   ```
   *Expected Current Output*: Fails on `ConnectionClosedError (1009 message too big)` and timeout.

4. **Verify Rust Core & Daemon pass**:
   ```bash
   cargo check --all-targets
   cargo test -p aura-core
   cargo test --manifest-path deploy/images/vsock-shell-daemon/Cargo.toml
   ```
   *Expected Current Output*: All compile cleanly and pass 100%.

5. **Invalidation Condition**:
   This audit finding is invalidated when `bash.py` and `browser.py` are patched and `pytest aura-sdk/tests` succeeds with 0 failures across all test files.
