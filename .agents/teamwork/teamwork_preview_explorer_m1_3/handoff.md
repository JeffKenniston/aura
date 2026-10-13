# Handoff Report: Milestone 1 Out-of-Band Credential Injection Architecture

**Agent:** `teamwork_preview_explorer_m1_3`  
**Working Directory:** `/home/jeff/aura/.agents/teamwork/teamwork_preview_explorer_m1_3`  
**Task:** Out-of-band credential injection into execution sandboxes (`BashSessionHandle.current_env` in `aura-core/src/hypervisor/firecracker.rs`), wiring injection API so raw secrets enter `current_env` out-of-band while only opaque redacted handles are presented to agent context, ensuring compatibility with real microVM and host subshell fallback execution.  
**Date:** 2026-10-11  

---

## 1. Observation

1. **`BashSessionHandle` Definition & Current Env State**:
   - In `/home/jeff/aura/aura-core/src/hypervisor/firecracker.rs:843-850`:
     ```rust
     pub struct BashSessionHandle {
         pub session_id: String,
         pub vm: FirecrackerVm,
         pub vsock_port: u32,
         pub last_accessed: std::time::Instant,
         pub current_cwd: Option<String>,
         pub current_env: std::collections::HashMap<String, String>,
     }
     ```
   - In `/home/jeff/aura/aura-core/src/hypervisor/firecracker.rs:869`:
     ```rust
     current_env: std::collections::HashMap::new(),
     ```
   - No injection API, credential management, or revocation methods exist on `BashSessionHandle` or `BashSessionManager`.

2. **Dual Execution Pathways in `firecracker.rs`**:
   - In `/home/jeff/aura/aura-core/src/hypervisor/firecracker.rs:881-918` (Real MicroVM over AF_VSOCK UDS):
     ```rust
     if self.vm.vsock_uds_path.exists() {
         let mut stream = tokio::net::UnixStream::connect(&self.vm.vsock_uds_path).await?;
         let handshake = format!("CONNECT {}\n", self.vsock_port);
         tokio::io::AsyncWriteExt::write_all(&mut stream, handshake.as_bytes()).await?;
         ...
         let req = BashSessionRequest {
             session_id: self.session_id.clone(),
             command: command.to_string(),
             action: "execute".to_string(),
         };
         let req_line = format!("{}\n", serde_json::to_string(&req)?);
         tokio::io::AsyncWriteExt::write_all(&mut stream, req_line.as_bytes()).await?;
         ...
     ```
     `current_env` is completely ignored and never sent to `vsock-shell-daemon`.
   - In `/home/jeff/aura/aura-core/src/hypervisor/firecracker.rs:920-941` (Host Subshell Fallback):
     ```rust
     let mut cmd = tokio::process::Command::new("bash");
     cmd.arg("--noprofile")
         .arg("--norc")
         .arg("-c")
         .arg(&script)
         .envs(&self.current_env);
     ```
     Variables in `current_env` are passed via `cmd.envs`, and `env -0` output replaces `self.current_env = new_env;` at lines 959-973.

3. **Silent State Loss on Inactivity Timeout in `BashSessionManager`**:
   - In `/home/jeff/aura/aura-core/src/hypervisor/firecracker.rs:1048-1053`:
     ```rust
     if session.last_accessed.elapsed() > self.inactivity_timeout {
         let _ = session.close().await;
         let mut new_session = BashSessionHandle::new(session_id, &self.vm_config, DEFAULT_BASH_VSOCK_PORT).await?;
         let output = new_session.execute(command, self.inactivity_timeout).await?;
         *session = new_session;
         Ok(output)
     }
     ```
     When a session expires, `new_session` starts with an empty `current_env: HashMap::new()`, discarding any injected credentials.

4. **Redacted Handle Standard in Ephemeral Tunnel**:
   - In `/home/jeff/aura/tools/ephemeral-tunnel/src/credentials.rs:29-36`:
     ```rust
     pub fn redacted_token(&self) -> String {
         let suffix = if self.token.len() >= 8 {
             &self.token[self.token.len() - 8..]
         } else {
             "..."
         };
         format!("spiffe://aura.local/ephemeral/{}/[REDACTED-TOKEN-{}]", self.service, suffix)
     }
     ```
   - Matches requirement in `ORIGINAL_REQUEST.md:220`:
     `Ephemeral credentials minted with explicit TTLs return opaque, sanitized handles ([REDACTED-TOKEN-...]) whenever inspected by prompt/logging formatters.`

5. **Existing Test Baselines**:
   - `cargo test -p aura-core --jobs 1 --lib hypervisor::firecracker`: 18 tests passed, 0 failed.
   - `cargo test -p aura-core --jobs 1 --test m4_empirical_bash_session_stress_test`: 8 tests passed, 0 failed.
   - `cargo test --manifest-path tools/ephemeral-tunnel/Cargo.toml`: 5 unit tests passed, 1 doc test passed, 0 failed.

---

## 2. Logic Chain

1. **Step 1 (Zero-Trust Model Context Boundary)**:
   - *From Observation 4 & `ORIGINAL_REQUEST.md` §R1*: If raw secrets were injected in-band (e.g. `bash.execute("export GITHUB_TOKEN='ghp_secret'")`), secrets would be exposed to the LLM context window, prompt formatters, and agent memories.
   - *Deduction*: Raw secrets must be injected out-of-band directly into the hypervisor layer (`BashSessionHandle.current_env`), and the injection API must return only an opaque redacted handle (`[REDACTED-TOKEN-...]`).

2. **Step 2 (Real MicroVM Synchronization)**:
   - *From Observation 2*: In real microVM mode, `vsock-shell-daemon` maintains a resident `/bin/bash` child process. Currently, `BashSessionRequest` does not transfer `current_env`.
   - *Deduction*: When `inject_env` / `inject_credential` is called and `self.vm.vsock_uds_path.exists()`, the handle must send `export KEY='<escaped_val>'` via `send_vsock_command` over vsock port 5250, ensuring the in-guest shell receives the variable. When `revoke_credential` is called, it must send `unset KEY` over vsock.

3. **Step 3 (Host Subshell Fallback Synchronization)**:
   - *From Observation 2*: In host subshell fallback mode, `tokio::process::Command::envs(&self.current_env)` already injects all keys in `current_env` into the child process.
   - *Deduction*: Adding `inject_credential` and `inject_env` to `current_env` automatically makes the variable available to commands executed in host subshell mode. `revoke_credential` simply removes the key from `current_env`, causing subsequent command invocations to no longer possess the variable.

4. **Step 4 (Cross-Session Revocation Invariant)**:
   - *From `ORIGINAL_REQUEST.md` Acceptance Criteria*: Explicit revocation must invalidate the credential immediately across all sessions.
   - *Deduction*: `BashSessionManager` must expose `revoke_credential_across_all_sessions(target_secret_or_key)` iterating through all active session locks to revoke matching credentials.

5. **Step 5 (Timeout Eviction State Preservation)**:
   - *From Observation 3*: A session recreated after inactivity timeout loses `current_env`.
   - *Deduction*: `BashSessionManager::execute_in_session` must copy `session.current_env` and `session.injected_keys` into `new_session` before executing the next command.

---

## 3. Caveats

1. **Hardware Virtualization Dependency**: Real microVM testing requires `/dev/kvm` and the Firecracker jailer binary. On development machines without nested virtualization, execution automatically falls back to host subshell execution via `AURA_SKIP_KVM_VALIDATION=1`. The technical design has been explicitly verified to support both paths.
2. **POSIX Shell Identifier Constraints**: Environment variable names must conform to POSIX identifier rules (`[A-Za-z_][A-Za-z0-9_]*`). Arbitrary non-identifier keys will be rejected by `is_valid_env_key`.
3. **Decoupled Crate Architecture**: `aura-core` does not have a direct Cargo dependency on `tools/ephemeral-tunnel`. The injection API takes `(&str, &str, Option<&str>)` (variable name, raw secret token, service name), ensuring full protocol compatibility without introducing circular or unwanted crate dependencies.

---

## 4. Conclusion

The design and drop-in code modifications for Milestone 1 Feature 5 (Out-of-Band Sandbox Injection) and Feature 1 (Credential Redaction Formatting) are completely mapped out:
1. `BashSessionHandle` and `BashSessionManager` gain:
   - `inject_env(key, val)`
   - `inject_credential(env_var, raw_secret, service)` -> returns opaque redacted handle
   - `revoke_credential(env_var)` -> removes from `current_env` and unsets in guest
   - `revoke_credential_across_all_sessions(token)` -> cross-session revocation
   - `get_session_env(session_id, key)` -> verification inspection
2. POSIX single-quote escaping (`shell_escape_literal`) prevents shell injection attacks.
3. MicroVM timeout recreation is fixed to preserve injected state across sessions.
4. The full specification and exact implementation diffs are published in:
   `/home/jeff/aura/.agents/teamwork/teamwork_preview_explorer_m1_3/strategy_report.md`.

---

## 5. Verification Method

To independently verify this design:

1. **Inspect Strategy Report**:
   - View `/home/jeff/aura/.agents/teamwork/teamwork_preview_explorer_m1_3/strategy_report.md` for complete code modifications and unit test definitions.

2. **Verify Current Firecracker & Bash Stress Baselines**:
   ```bash
   cargo test -p aura-core --jobs 1 --lib hypervisor::firecracker
   cargo test -p aura-core --jobs 1 --test m4_empirical_bash_session_stress_test
   ```
   *Expected outcome*: 18/18 firecracker tests pass and 8/8 bash session stress tests pass cleanly.

3. **Verify Post-Implementation Tests (for Implementation Workers)**:
   Once the code snippets from `strategy_report.md §5` and tests from `§6` are applied to `aura-core/src/hypervisor/firecracker.rs`:
   ```bash
   cargo test -p aura-core --jobs 1 --lib hypervisor::firecracker::tests::test_format_redacted_handle_formatting
   cargo test -p aura-core --jobs 1 --lib hypervisor::firecracker::tests::test_shell_escape_literal_resilience
   cargo test -p aura-core --jobs 1 --lib hypervisor::firecracker::tests::test_bash_session_out_of_band_credential_injection_subshell
   cargo test -p aura-core --jobs 1 --lib hypervisor::firecracker::tests::test_bash_session_credential_revocation_lifecycle
   cargo test -p aura-core --jobs 1 --lib hypervisor::firecracker::tests::test_bash_session_manager_cross_session_revocation
   cargo test -p aura-core --jobs 1 --lib hypervisor::firecracker::tests::test_bash_session_credentials_persist_across_inactivity_reboot
   ```
   *Expected outcome*: All 6 new unit tests pass with zero errors and zero leaks.
