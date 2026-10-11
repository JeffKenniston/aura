# Empirical Challenger Handoff Report: Milestone 4 (TOOL-BASH-002)

**Challenger ID**: `challenger_m4_1`  
**Working Directory**: `/home/jeff/aura/.agents/teamwork/challenger_m4_1/`  
**Target Requirement**: `TOOL-BASH-002` (Stateful Sandboxed Bash Tool & In-Guest Daemon)  
**Parent Conversation ID**: `ad091f9f-df8d-4332-bf82-a7111f5328d2`  
**Verdict**: **REJECT** (4 Reproducible Empirical Defects Identified)  

---

## 1. Observation

### 1.1 Baseline Test Executions
The baseline unit test suites specified in the dispatch passed successfully:
1. `cargo test -p aura-core --lib hypervisor::firecracker::tests`
   - Command: `cargo test -p aura-core --lib hypervisor::firecracker::tests`
   - Result: `18 passed; 0 failed; 0 ignored; 0 measured; finished in 0.07s`
2. `cargo test --manifest-path deploy/images/vsock-shell-daemon/Cargo.toml`
   - Command: `cargo test --manifest-path deploy/images/vsock-shell-daemon/Cargo.toml`
   - Result: `8 passed; 0 failed; 0 ignored; 0 measured; finished in 0.01s`
3. `source aura-sdk/.venv/bin/activate && pytest aura-sdk/tests/test_bash_tool.py`
   - Command: `pytest aura-sdk/tests/test_bash_tool.py`
   - Result: `7 passed, 1 warning in 0.60s`

### 1.2 Empirical Stress Test Executions & Failure Observations

Two dedicated empirical stress test suites were authored and executed:
- `aura-sdk/tests/test_bash_stress.py` (12 tests)
- `aura-core/tests/m4_empirical_bash_session_stress_test.rs` (8 tests)

#### Observation 1: Working Directory Loss on Paths with Spaces
- **Files & Lines**:
  - `aura-sdk/aura_sdk/tools/bash.py:71`:
    ```python
    script = (
        f"cd {session['cwd']} 2>/dev/null || true\n"
        f"{command}\n"
        ...
    )
    ```
  - `aura-core/src/hypervisor/firecracker.rs:928-931`:
    ```rust
    let script = format!(
        "cd {} 2>/dev/null || true\n{}\n__AURA_STATUS__=$?\nprintf \"\\n{}:%s\\n{}:%s\\n{}\\n\" \"$PWD\" \"$__AURA_STATUS__\" \"{}\"\nenv -0\n",
        cwd_clause, command, sentinel_cwd, sentinel_exit, sentinel_env, sentinel_env
    );
    ```
- **Verbatim Failure (Python)**:
  ```text
  FAILED aura-sdk/tests/test_bash_stress.py::test_adversarial_space_in_working_directory
  AssertionError: Expected cwd to persist as /tmp/aura space test qe1_0mqt, but got /home/jeff/aura
  assert '/home/jeff/aura' == '/tmp/aura sp...test qe1_0mqt'
  ```
- **Verbatim Failure (Rust)**:
  ```text
  failures:
  ---- test_bash_session_adversarial_cwd_with_spaces stdout ----
  thread 'test_bash_session_adversarial_cwd_with_spaces' panicked at aura-core/tests/m4_empirical_bash_session_stress_test.rs:211:5:
  assertion `left == right` failed: Working directory with spaces should persist across calls
    left: "/home/jeff/aura/aura-core"
   right: "/tmp/aura test space bnk5Vz"
  ```

#### Observation 2: Inability to Unset Environment Variables (Permanent Variable Retention)
- **Files & Lines**:
  - `aura-sdk/aura_sdk/tools/bash.py:103-107`:
    ```python
    for item in env_part.split(b"\x00"):
        if b"=" in item:
            k, v = item.split(b"=", 1)
            session["env"][k.decode("utf-8", errors="replace")] = v.decode("utf-8", errors="replace")
    ```
  - `aura-core/src/hypervisor/firecracker.rs:960-969`:
    ```rust
    for entry in post_trimmed.split(|&b| b == 0) {
        if let Some(eq_idx) = entry.iter().position(|&b| b == b'=') {
            if let (Ok(k), Ok(v)) = (
                std::str::from_utf8(&entry[..eq_idx]),
                std::str::from_utf8(&entry[eq_idx + 1..]),
            ) {
                self.current_env.insert(k.to_string(), v.to_string());
            }
        }
    }
    ```
- **Verbatim Failure (Python)**:
  ```text
  FAILED aura-sdk/tests/test_bash_stress.py::test_adversarial_unset_environment_variable
  AssertionError: Expected UNSET_ME to be cleared, but got [present]
  assert '[present]' == '[]'
  ```
- **Verbatim Failure (Rust)**:
  ```text
  failures:
  ---- test_bash_session_adversarial_unset_env stdout ----
  thread 'test_bash_session_adversarial_unset_env' panicked at aura-core/tests/m4_empirical_bash_session_stress_test.rs:241:5:
  assertion `left == right` failed: Environment variable should be cleared after unset
    left: "[present]"
   right: "[]"
  ```

#### Observation 3: Silent Stderr Swallowing on Command Failures in Python SDK
- **Files & Lines**:
  - `aura-sdk/aura_sdk/tools/bash.py:73-78, 109-111`:
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
- **Verbatim Failure (Python)**:
  ```text
  FAILED aura-sdk/tests/test_bash_stress.py::test_adversarial_stderr_capture_on_command_failure
  AssertionError: Expected stderr output on failure, but got empty string
  assert '' != ''
  ```

#### Observation 4: Global Mutex Lock Held During Command Execution Serializes All Sessions in Rust
- **Files & Lines**:
  - `aura-core/src/hypervisor/firecracker.rs:1032-1049`:
    ```rust
    pub async fn execute_in_session(
        &self,
        session_id: &str,
        command: &str,
    ) -> Result<BashCommandOutput, Box<dyn std::error::Error + Send + Sync>> {
        let mut sessions = self.sessions.lock().await;
        if let Some(session) = sessions.get_mut(session_id) {
            ...
            session.execute(command, self.inactivity_timeout).await
        } else {
            ...
            let output = session.execute(command, self.inactivity_timeout).await?;
            ...
        }
    }
    ```
- **Verbatim Failure (Rust)**:
  ```text
  failures:
  ---- test_bash_session_cross_session_blocking_lock stdout ----
  Two 1-second commands across different sessions took: 2.021845471s
  thread 'test_bash_session_cross_session_blocking_lock' panicked at aura-core/tests/m4_empirical_bash_session_stress_test.rs:271:5:
  Sessions should execute concurrently; holding global sessions lock serializes them: elapsed 2.021845471s
  ```

---

## 2. Logic Chain

1. **Working Directory State Invalidation (Defect 1)**:
   - When a command changes directory to a path with spaces (e.g. `mkdir "/tmp/aura space dir" && cd "/tmp/aura space dir"`), `pwd` correctly returns the path with spaces, and the session stores it.
   - On the next command invocation, both Python (`bash.py:71`) and Rust (`firecracker.rs:929`) construct the preamble script with unquoted `cd {cwd}`.
   - In bash, `cd /tmp/aura space dir` receives multiple arguments (`/tmp/aura`, `space`, `dir`), producing `bash: cd: too many arguments`.
   - The error is discarded via `2>/dev/null || true`.
   - The shell process remains in the parent directory (`/home/jeff/aura`), and the new cwd is captured as `/home/jeff/aura`, permanently destroying the user's working directory state.

2. **Environment Variable Accumulation / Unset Leak (Defect 2)**:
   - When an environment variable is exported, it is inserted into the session's internal dictionary/map.
   - When the user runs `unset VAR`, the command executes, and `env -0` correctly omits `VAR`.
   - However, when the script parses the `env -0` output, it iterates through each key-value pair and calls `session["env"][k] = v` (Python) or `self.current_env.insert(k, v)` (Rust).
   - Neither implementation resets or clears keys not present in `env -0`.
   - On the subsequent command invocation, the session passes its internal map (`session["env"]`) to the subprocess or Command builder.
   - As a result, the old value of `VAR` is re-injected into the subshell, making it impossible to unset any environment variable.

3. **Stderr Swallowing / Dead Error Branch (Defect 3)**:
   - In `bash.py`, the shell script terminates with `echo -n '__AURA_ENV_MARKER__'\nenv -0\n`.
   - The subprocess exit code `proc.returncode` is determined solely by the last command in the pipeline script (`env -0`).
   - Because `env -0` succeeds with exit code 0, `proc.returncode` is always 0.
   - The error check `if proc.returncode != 0 and stderr_bytes and not output_str:` is unreachable dead code.
   - Any failing command that writes to stderr and produces no stdout returns `output_str` (which is `""`), completely hiding the failure and stderr message from the agent.

4. **Global Mutex Serialization (Defect 4)**:
   - In `BashSessionManager`, `self.sessions` is an `Arc<tokio::sync::Mutex<HashMap<String, BashSessionHandle>>>`.
   - In `execute_in_session`, the mutex guard `let mut sessions = self.sessions.lock().await;` is kept in scope across the `session.execute().await` call.
   - Because `.await` yields while holding the mutex guard on the entire hash map, any other task calling `execute_in_session` for any session must wait until the current command completes.
   - This destroys execution parallelism across sessions and causes global throughput starvation.

---

## 3. Caveats

- **In-Guest Daemon Isolation**: In `deploy/images/vsock-shell-daemon`, the resident `/bin/bash` process handles `unset` and directory paths with spaces correctly because bash maintains its own resident memory state and `ensure_alive` quotes `current_cwd` (`cd "{}"`). The issues in Defect 1 and Defect 2 occur in host subshell fallback execution mode (`firecracker.rs:920-999`) and Python SDK standalone mode (`bash.py:64-114`).
- **KVM Virtualization in Container**: Host KVM `/dev/kvm` is not exposed in the development container, so Firecracker microVMs operate in mock/fallback mode as verified in Milestone 1 and 2.

---

## 4. Conclusion

**Verdict: REJECT**

Milestone 4's implementation of `TOOL-BASH-002` contains 4 verifiable, empirically reproducible defects that break state continuity, environment cleanup, error propagation, and multi-session concurrency:
1. Working directory paths with spaces fail to persist across consecutive calls.
2. Unsetting environment variables (`unset VAR`) fails; previous values are re-injected.
3. Python `BashTool` swallows stderr output on command failures, returning an empty string instead of the error.
4. Rust `BashSessionManager` serializes all command executions across distinct sessions via a global mutex lock held across async I/O.

---

## 5. Verification Method

### 5.1 Verification Commands
To independently reproduce all findings:

1. **Python SDK Stress Test (Defects 1, 2, 3)**:
   ```bash
   source aura-sdk/.venv/bin/activate && pytest aura-sdk/tests/test_bash_stress.py
   ```
   *Expected Result*: 3 failed (`test_adversarial_space_in_working_directory`, `test_adversarial_unset_environment_variable`, `test_adversarial_stderr_capture_on_command_failure`), 9 passed.

2. **Rust Core Stress Test (Defects 1, 2, 4)**:
   ```bash
   cargo test -p aura-core --test m4_empirical_bash_session_stress_test
   ```
   *Expected Result*: 3 failed (`test_bash_session_adversarial_cwd_with_spaces`, `test_bash_session_adversarial_unset_env`, `test_bash_session_cross_session_blocking_lock`), 5 passed.

### 5.2 Required Remediations
1. **Quote `cd` target**:
   - In `aura-sdk/aura_sdk/tools/bash.py:71`: Change to `f'cd "{session[\"cwd\"]}" 2>/dev/null || true\n'`
   - In `aura-core/src/hypervisor/firecracker.rs:929`: Change to `cd "{}" 2>/dev/null || true\n`
2. **Synchronize environment map**:
   - In `bash.py`: Replace `session["env"]` with a fresh dictionary parsed from `env -0` rather than mutating the existing one.
   - In `firecracker.rs`: Clear `self.current_env.clear()` or build a new map from `env -0` before assigning.
3. **Capture and return exit code and stderr**:
   - In `bash.py`: Output `__AURA_EXIT__` alongside markers, parse it, and return `stderr_bytes.decode()` when exit code is non-zero and user stdout is empty.
4. **Per-session locking**:
   - In `BashSessionManager`: Do not hold the global `self.sessions` lock across the execution of a command. Store `Arc<tokio::sync::Mutex<BashSessionHandle>>` or release the map lock before awaiting `session.execute()`.
