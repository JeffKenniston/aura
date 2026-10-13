# Technical Design & Fix Strategy Report: Milestone 1 Out-of-Band Credential Injection

**Target Artifact:** `aura-core/src/hypervisor/firecracker.rs` & `aura-core/src/hypervisor/mod.rs`  
**Milestone Focus:** Phase M5 / Milestone 1 — Feature 5 (Out-of-Band Sandbox Injection) & Feature 1 (Credential Redaction Formatting)  
**Author:** `teamwork_preview_explorer_m1_3`  
**Date:** 2026-10-11  

---

## 1. Executive Summary

In Aura OS, autonomous agents execute shell operations inside sandboxed environments managed by `BashSessionManager` and `BashSessionHandle` in `aura-core/src/hypervisor/firecracker.rs`. Under Phase M5 Requirement R1 (Ephemeral Credential Lifecycle & Out-of-Band Secret Isolation), the microkernel must mint short-lived, task-scoped credentials and inject them **out-of-band directly into target execution sandboxes** (`BashSessionHandle.current_env`) without exposing raw secrets to model context, agent prompts, or tool execution history.

Currently, `BashSessionHandle.current_env` is initialized to an empty `HashMap`, and the codebase lacks an injection API. In addition:
1. When operating in **real microVM mode** (via Firecracker `AF_VSOCK` port 5250), `BashSessionRequest` does not communicate environment variables to the resident guest shell daemon (`vsock-shell-daemon`), meaning variables set on the host handle are not present in guest shell processes.
2. When operating in **host subshell fallback mode**, commands inherit `current_env` through `tokio::process::Command::envs`, but there is no mechanism to inject credentials, return sanitized handles, or revoke credentials across active sessions.
3. When microVM instances expire after the 125-second inactivity timeout, `BashSessionManager` creates a fresh `BashSessionHandle::new(...)` which resets `current_env` to an empty map, silently dropping any injected credentials.

This report establishes the complete technical design, exact code modifications, and verification plan to implement out-of-band credential injection, opaque redacted handle presentation, cross-session revocation, and dual-mode runtime compatibility.

---

## 2. Problem Framing & Threat Model

### 2.1 In-Band vs. Out-of-Band Credential Flow

| Dimension | In-Band Injection (Vulnerable / Anti-Pattern) | Out-of-Band Injection (Aura Zero-Trust Invariant) |
|---|---|---|
| **Payload Delivery** | Raw secret token is embedded directly in prompt or shell command text (e.g. `export GITHUB_TOKEN='ghp_secret'`). | Host hypervisor injects raw secret directly into `BashSessionHandle.current_env` via internal API. |
| **Model Context Exposure** | Raw secret token appears in LLM input prompt, tool calls, context window, and agent memories. | Agent context and prompt formatters receive ONLY an opaque sanitized handle (e.g. `spiffe://aura.local/ephemeral/github/[REDACTED-TOKEN-ey_12345]`). |
| **Leakage Surface** | Context dumps, token replay, prompt injection attacks, and agent message history leak raw credentials. | Even full prompt/log dumps reveal zero secrets; raw secrets exist solely in sandboxed process memory. |
| **Revocation Control** | Cannot reliably revoke credentials if persisted in agent state or command history. | Explicit revocation immediately unsets the variable across all active sandbox sessions (`unset GITHUB_TOKEN`) and invalidates the token in `EphemeralTunnelService`. |

### 2.2 Invariants Enforced
- **Zero Raw Secret Exposure**: Any method accepting a secret token returns only an opaque redacted handle to the caller.
- **Immediate Cross-Session Revocation**: When a token is revoked, all sessions containing the credential remove it immediately.
- **Dual Runtime Support**: Operates identically whether running against a physical KVM Firecracker microVM with `vsock-shell-daemon` or falling back to host subshell execution in test/development environments.
- **Timeout Continuity**: If a microVM instance is respawned after an inactivity timeout, all active injected credentials are automatically restored in the new session.

---

## 3. Detailed Technical Architecture

### 3.1 Architectural Workflow Diagram

```
                 +---------------------------------------------+
                 |  EphemeralTunnelService / Caller Workflow   |
                 +---------------------------------------------+
                                        |
                 1. mint_credentials()  | (Returns raw token & metadata)
                                        v
                 +---------------------------------------------+
                 |      Out-of-Band Sandbox Injection API      |
                 |  session_manager.inject_credential(         |
                 |      sid, "GITHUB_TOKEN", raw_token, svc)   |
                 +---------------------------------------------+
                        |                               |
  (Returns Redacted)    |                               | (Direct Out-of-Band
   Opaque Handle        |                               |  Environment Write)
                        v                               v
         +-----------------------------+   +-----------------------------+
         |    Agent / Model Context    |   |     BashSessionHandle       |
         | [REDACTED-TOKEN-ey_12345]   |   | current_env["GITHUB_TOKEN"] |
         +-----------------------------+   |  = "ghp_raw_secret_xyz..."  |
                                           +-----------------------------+
                                                          |
                                           +--------------+--------------+
                                           |                             |
                                           v                             v
                           [Real Firecracker MicroVM]     [Host Subshell Fallback]
                           - Sends `export KEY='val'`     - Passes `cmd.envs(&current_env)`
                             over vsock port 5250           to spawned subshell
                           - Resident bash child process  - Evaluates `$GITHUB_TOKEN`
                             receives variable securely     authentically
```

### 3.2 Opaque Redacted Handle Generation

To conform with `PROJECT.md` Feature 1 and `tools/ephemeral-tunnel/src/credentials.rs`:
- When a service name is supplied: `spiffe://aura.local/ephemeral/{service}/[REDACTED-TOKEN-{suffix}]`
- When no service name is supplied: `[REDACTED-TOKEN-{suffix}]`
- Where `suffix` is the last 8 characters of the raw token (or `...` if length < 8).

```rust
pub fn format_redacted_handle(raw_token: &str, service: Option<&str>) -> String {
    let suffix = if raw_token.len() >= 8 {
        &raw_token[raw_token.len() - 8..]
    } else {
        "..."
    };
    match service {
        Some(s) if !s.is_empty() => {
            format!("spiffe://aura.local/ephemeral/{}/[REDACTED-TOKEN-{}]", s, suffix)
        }
        _ => format!("[REDACTED-TOKEN-{}]", suffix),
    }
}
```

### 3.3 Shell Escaping Security Primitives

To prevent arbitrary command execution or escape sequences during out-of-band export commands:
1. **Identifier Validation**: Verifies that environment variable names match `^[A-Za-z_][A-Za-z0-9_]*$`.
2. **POSIX Single-Quote Escaping**: Wraps values in single quotes (`'...'`), replacing any embedded single quote with `'\''`. Inside POSIX single quotes, parameter expansion, command substitution, arithmetic expansion, and backslash processing are completely disabled.

```rust
pub fn shell_escape_literal(s: &str) -> String {
    let mut escaped = String::with_capacity(s.len() + 2);
    escaped.push('\'');
    for c in s.chars() {
        if c == '\'' {
            escaped.push_str("'\\''");
        } else {
            escaped.push(c);
        }
    }
    escaped.push('\'');
    escaped
}

pub fn is_valid_env_key(key: &str) -> bool {
    if key.is_empty() {
        return false;
    }
    let mut chars = key.chars();
    let first = chars.next().unwrap();
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}
```

---

## 4. Execution Mode Compatibility Design

### 4.1 Comparison of Execution Pathways

| Phase | Real MicroVM Mode (`vsock_uds_path.exists()`) | Host Subshell Fallback Mode |
|---|---|---|
| **Substrate** | Firecracker MicroVM jailer with KVM acceleration | Local `/bin/bash` process with subshell environment |
| **Daemon / Process** | In-guest `vsock-shell-daemon` listening on vsock 5250 with persistent resident bash child | Process spawned on-demand per command with `--noprofile --norc -c` |
| **Injection Mechanism** | Sends `export KEY='...'` via `send_vsock_command` to resident shell; records in `current_env` | Records in `current_env`; passed to child via `tokio::process::Command::envs` |
| **Command Execution** | Sends `BashSessionRequest` over Unix stream (`CONNECT 5250\n`) | Executes script with sentinel exit/cwd markers; extracts modified env via `env -0` |
| **Revocation Mechanism** | Sends `unset KEY` over vsock stream; removes from `current_env` | Removes from `current_env`; subsequent commands no longer receive variable |
| **Inactivity Eviction** | Closes microVM after 125s; on next execution, boots new microVM and restores `current_env` | Resets handle after 125s; restores `current_env` into new session handle |

### 4.2 Inactivity Timeout Session Recreation Bug Fix

In `BashSessionManager::execute_in_session`, lines 1048-1054 currently drop all session state on timeout:
```rust
// CURRENT DEFECTIVE CODE:
if session.last_accessed.elapsed() > self.inactivity_timeout {
    let _ = session.close().await;
    let mut new_session = BashSessionHandle::new(session_id, &self.vm_config, DEFAULT_BASH_VSOCK_PORT).await?;
    let output = new_session.execute(command, self.inactivity_timeout).await?;
    *session = new_session;
    Ok(output)
}
```
**Fix**: Preserve `session.current_env` and `session.injected_keys` when creating `new_session`:
```rust
// REMEDIATED CODE:
if session.last_accessed.elapsed() > self.inactivity_timeout {
    let old_env = session.current_env.clone();
    let old_keys = session.injected_keys.clone();
    let _ = session.close().await;
    let mut new_session = BashSessionHandle::new(session_id, &self.vm_config, DEFAULT_BASH_VSOCK_PORT).await?;
    new_session.current_env = old_env;
    new_session.injected_keys = old_keys;
    let output = new_session.execute(command, self.inactivity_timeout).await?;
    *session = new_session;
    Ok(output)
}
```

---

## 5. Exact Code Modifications

### 5.1 Modifications in `aura-core/src/hypervisor/firecracker.rs`

#### Modification 1: Helper Functions & Redacted Handle Generation
*Location:* Add before `BashCommandOutput` (around line 832).

```rust
/// Formats a raw token into an opaque, sanitized handle for model context and logs (TOOL-ET-002).
pub fn format_redacted_handle(raw_token: &str, service: Option<&str>) -> String {
    let suffix = if raw_token.len() >= 8 {
        &raw_token[raw_token.len() - 8..]
    } else {
        "..."
    };
    match service {
        Some(s) if !s.is_empty() => {
            format!("spiffe://aura.local/ephemeral/{}/[REDACTED-TOKEN-{}]", s, suffix)
        }
        _ => format!("[REDACTED-TOKEN-{}]", suffix),
    }
}

/// Safely escapes a string literal for POSIX single-quoted shell execution (`'...'`).
/// Guarantees zero shell injection when propagating variables over vsock or subshell.
pub fn shell_escape_literal(s: &str) -> String {
    let mut escaped = String::with_capacity(s.len() + 2);
    escaped.push('\'');
    for c in s.chars() {
        if c == '\'' {
            escaped.push_str("'\\''");
        } else {
            escaped.push(c);
        }
    }
    escaped.push('\'');
    escaped
}

/// Validates that an environment variable key is a valid POSIX identifier ([A-Za-z_][A-Za-z0-9_]*).
pub fn is_valid_env_key(key: &str) -> bool {
    if key.is_empty() {
        return false;
    }
    let mut chars = key.chars();
    let first = chars.next().unwrap();
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}
```

#### Modification 2: `BashSessionHandle` Struct Extension
*Location:* Lines 843-850. Add `pub injected_keys: std::collections::HashSet<String>`.

```rust
/// Handle managing a persistent microVM shell session (TOOL-BASH-002).
pub struct BashSessionHandle {
    pub session_id: String,
    pub vm: FirecrackerVm,
    pub vsock_port: u32,
    pub last_accessed: std::time::Instant,
    pub current_cwd: Option<String>,
    pub current_env: std::collections::HashMap<String, String>,
    pub injected_keys: std::collections::HashSet<String>,
}
```

In `BashSessionHandle::new`:
```rust
        Ok(Self {
            session_id: session_id.to_string(),
            vm,
            vsock_port,
            last_accessed: std::time::Instant::now(),
            current_cwd: None,
            current_env: std::collections::HashMap::new(),
            injected_keys: std::collections::HashSet::new(),
        })
```

#### Modification 3: Vsock Dispatch Helper on `BashSessionHandle`
*Location:* Add to `impl BashSessionHandle`.

```rust
    /// Dispatches a command to the in-guest shell daemon over Firecracker AF_VSOCK Unix stream.
    async fn send_vsock_command(
        &mut self,
        command: &str,
    ) -> Result<BashCommandOutput, Box<dyn std::error::Error + Send + Sync>> {
        let mut stream = tokio::net::UnixStream::connect(&self.vm.vsock_uds_path).await?;
        let handshake = format!("CONNECT {}\n", self.vsock_port);
        tokio::io::AsyncWriteExt::write_all(&mut stream, handshake.as_bytes()).await?;

        let mut handshake_resp = Vec::new();
        let mut b = [0u8; 1];
        while b[0] != b'\n' {
            tokio::io::AsyncReadExt::read_exact(&mut stream, &mut b).await?;
            handshake_resp.push(b[0]);
            if handshake_resp.len() > 256 {
                return Err("Vsock handshake response too long".into());
            }
        }
        let resp_str = String::from_utf8_lossy(&handshake_resp);
        if !resp_str.starts_with("OK") {
            return Err(format!("Vsock connection failed: {}", resp_str.trim()).into());
        }

        let req = BashSessionRequest {
            session_id: self.session_id.clone(),
            command: command.to_string(),
            action: "execute".to_string(),
        };
        let req_line = format!("{}\n", serde_json::to_string(&req)?);
        tokio::io::AsyncWriteExt::write_all(&mut stream, req_line.as_bytes()).await?;

        let mut reader = tokio::io::BufReader::new(stream);
        let mut resp_line = String::new();
        tokio::io::AsyncBufReadExt::read_line(&mut reader, &mut resp_line).await?;

        let output: BashCommandOutput = serde_json::from_str(resp_line.trim())?;
        if let Some(ref cwd) = output.cwd {
            self.current_cwd = Some(cwd.clone());
        }
        Ok(output)
    }
```

#### Modification 4: Out-of-Band Injection & Revocation Methods on `BashSessionHandle`
*Location:* Inside `impl BashSessionHandle`.

```rust
    /// Injects an environment variable into this session's sandbox out-of-band.
    ///
    /// Updates `self.current_env` and `self.injected_keys`. If running in real
    /// microVM mode (`vsock_uds_path.exists()`), the variable is immediately
    /// exported in the guest shell session via vsock.
    pub async fn inject_env(
        &mut self,
        key: &str,
        value: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !is_valid_env_key(key) {
            return Err(format!("Invalid environment variable identifier: '{}'", key).into());
        }
        self.last_accessed = std::time::Instant::now();
        self.current_env.insert(key.to_string(), value.to_string());
        self.injected_keys.insert(key.to_string());

        if self.vm.vsock_uds_path.exists() {
            let export_cmd = format!("export {}={}\n", key, shell_escape_literal(value));
            let _ = self.send_vsock_command(&export_cmd).await?;
        }

        Ok(())
    }

    /// Injects an ephemeral credential out-of-band into the session sandbox environment.
    ///
    /// The raw secret enters `self.current_env` out-of-band and is never leaked to
    /// caller logs or agent context. Returns an opaque sanitized handle (e.g.,
    /// `spiffe://aura.local/ephemeral/{service}/[REDACTED-TOKEN-{suffix}]` or `[REDACTED-TOKEN-{suffix}]`),
    /// which can be presented safely to the model context.
    pub async fn inject_credential(
        &mut self,
        env_var: &str,
        raw_secret: &str,
        service: Option<&str>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        if raw_secret.is_empty() {
            return Err("Cannot inject empty credential secret".into());
        }
        self.inject_env(env_var, raw_secret).await?;
        let handle = format_redacted_handle(raw_secret, service);
        Ok(handle)
    }

    /// Explicitly revokes and removes a credential / environment variable from this session.
    ///
    /// In host subshell mode, the key is removed from `current_env` so subsequent
    /// commands do not inherit it. In microVM mode, `unset <key>` is also executed
    /// in the guest shell over vsock.
    pub async fn revoke_credential(
        &mut self,
        env_var: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        self.last_accessed = std::time::Instant::now();
        self.injected_keys.remove(env_var);
        let removed = self.current_env.remove(env_var).is_some();

        if self.vm.vsock_uds_path.exists() {
            let unset_cmd = format!("unset {}\n", env_var);
            let _ = self.send_vsock_command(&unset_cmd).await?;
        }

        Ok(removed)
    }

    /// Returns whether the specified credential key is currently active in this session.
    pub fn is_credential_active(&self, env_var: &str) -> bool {
        self.injected_keys.contains(env_var) && self.current_env.contains_key(env_var)
    }

    /// Returns a reference to the environment value if present.
    pub fn get_env(&self, key: &str) -> Option<&String> {
        self.current_env.get(key)
    }
```

#### Modification 5: Out-of-Band Injection & Cross-Session Revocation on `BashSessionManager`
*Location:* Inside `impl BashSessionManager`.

```rust
    /// Injects an environment variable into the specified session out-of-band.
    /// Creates the session if it does not yet exist.
    pub async fn inject_env(
        &self,
        session_id: &str,
        key: &str,
        value: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let session_arc = self.get_or_create_session(session_id).await?;
        let mut session = session_arc.lock().await;
        session.inject_env(key, value).await
    }

    /// Injects an ephemeral credential out-of-band into the specified session.
    ///
    /// Raw secret enters `current_env` out-of-band; only the opaque redacted
    /// handle is returned to present to agent context (TOOL-ET-002, ORIGINAL_REQUEST §R1).
    pub async fn inject_credential(
        &self,
        session_id: &str,
        env_var: &str,
        raw_secret: &str,
        service: Option<&str>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let session_arc = self.get_or_create_session(session_id).await?;
        let mut session = session_arc.lock().await;
        session.inject_credential(env_var, raw_secret, service).await
    }

    /// Explicitly revokes a credential from a specific session.
    pub async fn revoke_credential(
        &self,
        session_id: &str,
        env_var: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let maybe_arc = {
            let sessions = self.sessions.lock().await;
            sessions.get(session_id).cloned()
        };
        if let Some(arc) = maybe_arc {
            let mut session = arc.lock().await;
            session.revoke_credential(env_var).await
        } else {
            Ok(false)
        }
    }

    /// Revokes credentials matching the raw secret or variable name across ALL active sessions.
    /// Fulfills acceptance criterion: "explicit revocation invalidates the credential immediately across all sessions".
    pub async fn revoke_credential_across_all_sessions(
        &self,
        target_token_or_var: &str,
    ) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        let session_arcs: Vec<_> = {
            let sessions = self.sessions.lock().await;
            sessions.values().cloned().collect()
        };

        let mut affected_count = 0;
        for arc in session_arcs {
            let mut session = arc.lock().await;
            let mut vars_to_revoke = Vec::new();

            for (k, v) in session.current_env.iter() {
                if k == target_token_or_var || v == target_token_or_var {
                    vars_to_revoke.push(k.clone());
                }
            }

            for var in vars_to_revoke {
                if session.revoke_credential(&var).await? {
                    affected_count += 1;
                }
            }
        }
        Ok(affected_count)
    }

    /// Helper to get or create a session Arc, minimizing lock contention.
    async fn get_or_create_session(
        &self,
        session_id: &str,
    ) -> Result<std::sync::Arc<tokio::sync::Mutex<BashSessionHandle>>, Box<dyn std::error::Error + Send + Sync>> {
        let mut sessions = self.sessions.lock().await;
        if let Some(arc) = sessions.get(session_id) {
            Ok(arc.clone())
        } else {
            let session = BashSessionHandle::new(session_id, &self.vm_config, DEFAULT_BASH_VSOCK_PORT).await?;
            let arc = std::sync::Arc::new(tokio::sync::Mutex::new(session));
            sessions.insert(session_id.to_string(), arc.clone());
            Ok(arc)
        }
    }

    /// Gets an environment variable value from a session, if present.
    pub async fn get_session_env(
        &self,
        session_id: &str,
        key: &str,
    ) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
        let maybe_arc = {
            let sessions = self.sessions.lock().await;
            sessions.get(session_id).cloned()
        };
        if let Some(arc) = maybe_arc {
            let session = arc.lock().await;
            Ok(session.get_env(key).cloned())
        } else {
            Ok(None)
        }
    }
```

### 5.2 Re-exports in `aura-core/src/hypervisor/mod.rs`
*Location:* Lines 5-6 in `aura-core/src/hypervisor/mod.rs`.

```rust
pub use firecracker::{
    format_redacted_handle, should_skip_kvm_validation, validate_kvm_at_path, validate_kvm_support,
    BashCommandOutput, BashSessionHandle, BashSessionManager, KvmError,
};
```

---

## 6. Comprehensive Verification Suite

### 6.1 Unit Tests for `firecracker.rs`

The following unit tests should be appended to `mod tests` in `aura-core/src/hypervisor/firecracker.rs`:

```rust
#[test]
fn test_format_redacted_handle_formatting() {
    let long_token = "spiffe://aura.local/ephemeral/github/550e8400-e29b-41d4-a716-446655440000";
    let formatted = format_redacted_handle(long_token, Some("github"));
    assert_eq!(
        formatted,
        "spiffe://aura.local/ephemeral/github/[REDACTED-TOKEN-440000]"
    );

    let default_formatted = format_redacted_handle(long_token, None);
    assert_eq!(default_formatted, "[REDACTED-TOKEN-440000]");

    let short_token = "abc";
    let short_formatted = format_redacted_handle(short_token, Some("vault"));
    assert_eq!(
        short_formatted,
        "spiffe://aura.local/ephemeral/vault/[REDACTED-TOKEN-...]"
    );
}

#[test]
fn test_shell_escape_literal_resilience() {
    assert_eq!(shell_escape_literal("plain"), "'plain'");
    assert_eq!(shell_escape_literal("with space"), "'with space'");
    assert_eq!(shell_escape_literal("can't stop"), "'can'\\''t stop'");
    assert_eq!(shell_escape_literal("$VAR; rm -rf /"), "'$VAR; rm -rf /'");
}

#[tokio::test]
async fn test_bash_session_out_of_band_credential_injection_subshell() {
    let vm_config = VmConfig::for_bash(None);
    let manager = BashSessionManager::new(vm_config, Duration::from_secs(125));
    let sid = "test-oob-inject";
    let secret = "ghp_SuperSecretCredentialString123456789";

    // 1. Inject credential out-of-band
    let handle = manager
        .inject_credential(sid, "GITHUB_TOKEN", secret, Some("github"))
        .await
        .expect("Injection failed");

    // 2. Assert handle is redacted and does NOT leak the secret
    assert!(handle.contains("[REDACTED-TOKEN-"));
    assert!(!handle.contains(secret));
    assert!(handle.ends_with("6789]"));

    // 3. Execute command asserting sandbox received raw secret out-of-band
    let output = manager
        .execute_in_session(sid, "echo $GITHUB_TOKEN")
        .await
        .expect("Execution failed");
    assert_eq!(output.stdout, secret);
    assert_eq!(output.exit_code, 0);

    // 4. Inspect session environment on host
    let env_val = manager
        .get_session_env(sid, "GITHUB_TOKEN")
        .await
        .expect("Failed to get env");
    assert_eq!(env_val, Some(secret.to_string()));
}

#[tokio::test]
async fn test_bash_session_credential_revocation_lifecycle() {
    let vm_config = VmConfig::for_bash(None);
    let manager = BashSessionManager::new(vm_config, Duration::from_secs(125));
    let sid = "test-oob-revoke";
    let secret = "secret_to_be_revoked_999";

    // 1. Inject credential
    let _ = manager
        .inject_credential(sid, "TEMP_AUTH", secret, None)
        .await
        .unwrap();

    let check1 = manager
        .execute_in_session(sid, "echo $TEMP_AUTH")
        .await
        .unwrap();
    assert_eq!(check1.stdout, secret);

    // 2. Explicitly revoke credential
    let revoked = manager.revoke_credential(sid, "TEMP_AUTH").await.unwrap();
    assert!(revoked);

    // 3. Subsequent execution confirms environment variable is cleared
    let check2 = manager
        .execute_in_session(sid, "echo \"[${TEMP_AUTH}]\"")
        .await
        .unwrap();
    assert_eq!(check2.stdout, "[]");

    let env_val = manager.get_session_env(sid, "TEMP_AUTH").await.unwrap();
    assert_eq!(env_val, None);
}

#[tokio::test]
async fn test_bash_session_manager_cross_session_revocation() {
    let vm_config = VmConfig::for_bash(None);
    let manager = BashSessionManager::new(vm_config, Duration::from_secs(125));
    let secret = "shared_ephemeral_token_xyz_8888";

    // Inject same secret into multiple sessions
    manager
        .inject_credential("session-alpha", "SHARED_KEY", secret, Some("service"))
        .await
        .unwrap();
    manager
        .inject_credential("session-beta", "SHARED_KEY", secret, Some("service"))
        .await
        .unwrap();
    manager
        .inject_credential("session-gamma", "OTHER_KEY", "unrelated_secret", None)
        .await
        .unwrap();

    // Invalidate across all sessions
    let revoked_count = manager
        .revoke_credential_across_all_sessions(secret)
        .await
        .unwrap();
    assert_eq!(revoked_count, 2);

    // Assert alpha and beta cleared, gamma untouched
    let check_alpha = manager
        .execute_in_session("session-alpha", "echo \"[${SHARED_KEY}]\"")
        .await
        .unwrap();
    assert_eq!(check_alpha.stdout, "[]");

    let check_beta = manager
        .execute_in_session("session-beta", "echo \"[${SHARED_KEY}]\"")
        .await
        .unwrap();
    assert_eq!(check_beta.stdout, "[]");

    let check_gamma = manager
        .execute_in_session("session-gamma", "echo $OTHER_KEY")
        .await
        .unwrap();
    assert_eq!(check_gamma.stdout, "unrelated_secret");
}

#[tokio::test]
async fn test_bash_session_credentials_persist_across_inactivity_reboot() {
    let vm_config = VmConfig::for_bash(None);
    let manager = BashSessionManager::new(vm_config, Duration::from_millis(50));
    let sid = "session-reboot-persist";
    let secret = "persistent_secret_across_timeouts_1234";

    manager
        .inject_credential(sid, "PERSIST_VAR", secret, None)
        .await
        .unwrap();

    // Sleep past inactivity timeout
    tokio::time::sleep(Duration::from_millis(70)).await;

    // Executing command triggers microVM recreation; credential must survive
    let out = manager
        .execute_in_session(sid, "echo $PERSIST_VAR")
        .await
        .unwrap();
    assert_eq!(
        out.stdout, secret,
        "Injected credentials must survive session expiration and microVM respawn"
    );
}
```

---

## 7. Peer Integration Matrix (Milestone 1)

| Subagent | Domain / Focus | Interaction Boundary with Out-of-Band Sandbox Injection |
|---|---|---|
| `spec_miner_m1_1` | `tools/ephemeral-tunnel` (`credentials.rs`, `lib.rs`) | Produces `Credentials` with custom `Display`/`Debug` redaction (`[REDACTED-TOKEN-...]`). The raw token from `Credentials.token` and `Credentials.service` is passed out-of-band to `BashSessionManager::inject_credential`. |
| `explorer_m1_2` | `aura-core/src/knowledge/git.rs` (`open_pr`, `EphemeralTokenGuard`) | In `GitManager::open_pr`, after minting ephemeral credentials with caller SVID validation, injects `token` out-of-band as `GITHUB_TOKEN` into the PR session. `EphemeralTokenGuard::drop` invokes `revoke_credential` on the session manager. |
| `explorer_m1_3` *(Author)* | `aura-core/src/hypervisor/firecracker.rs` (`BashSessionHandle`, `BashSessionManager`) | Implements `inject_credential`, `inject_env`, `revoke_credential`, and cross-session revocation. Guarantees raw secrets enter `current_env` while only redacted handles are returned. |

---

## 8. Concrete Actionable Next Steps for Implementation Workers

1. **Apply Drop-In Code Modifications to `aura-core/src/hypervisor/firecracker.rs`**:
   - Add `format_redacted_handle`, `shell_escape_literal`, and `is_valid_env_key`.
   - Add `injected_keys: HashSet<String>` to `BashSessionHandle`.
   - Add `send_vsock_command`, `inject_env`, `inject_credential`, `revoke_credential` to `BashSessionHandle`.
   - Add `inject_env`, `inject_credential`, `revoke_credential`, and `revoke_credential_across_all_sessions` to `BashSessionManager`.
   - Fix timeout recreation in `execute_in_session` to preserve `old_env` and `old_keys`.
2. **Update Re-Exports in `aura-core/src/hypervisor/mod.rs`**:
   - Export `format_redacted_handle`, `BashCommandOutput`, `BashSessionHandle`, `BashSessionManager`.
3. **Run Validation Commands**:
   ```bash
   cargo test -p aura-core --jobs 1 --lib hypervisor::firecracker
   cargo test -p aura-core --jobs 1 --test m4_empirical_bash_session_stress_test
   ```
4. **Coordinate Integration with `worker_m1_1` and `worker_m1_2`** for `open_pr` end-to-end integration.
