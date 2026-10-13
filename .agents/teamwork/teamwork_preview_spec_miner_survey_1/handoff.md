# Handoff Report — Phase M5 Requirement R1 Specification Mining

- **Agent**: `teamwork_preview_spec_miner_survey_1`
- **Working Directory**: `/home/jeff/aura/.agents/teamwork/teamwork_preview_spec_miner_survey_1`
- **Recipient**: Parent Orchestrator (`dae2eddf-f903-4d1f-b5f4-42a3e7019103`)
- **Target**: Requirement R1 (Ephemeral Credential Lifecycle & Out-of-Band Secret Isolation) for Phase M5 (Hardening)

---

## 1. Observation

1. **`tools/ephemeral-tunnel` Implementation & Formatting**:
   - `tools/ephemeral-tunnel/src/credentials.rs:4-8`:
     ```rust
     #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
     pub struct Credentials {
         pub token: String,
         pub expires_at: u64,
     }
     ```
     `Credentials` derives `Debug` directly; it does NOT implement `std::fmt::Display`. Calling `format!("{:?}", creds)` outputs the raw unredacted secret token (e.g., `spiffe://aura.local/ephemeral/vault/...`).
   - `tools/ephemeral-tunnel/src/credentials.rs:29-36`:
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
   - `tools/ephemeral-tunnel/src/spire.rs:162-171`:
     `sanitize_for_prompt(&self, token: &str) -> String` returns `[REDACTED-TOKEN]` or the redacted token string.
   - `tools/ephemeral-tunnel/src/lib.rs:18-30`:
     ```rust
     fn mint_credentials(service: String) -> Result<..., String> {
         let tunnel = EphemeralTunnelService::default();
         let creds = tunnel.mint_credentials(&service)?;
         ...
     }
     fn revoke_credentials(token: String) -> Result<(), String> {
         let tunnel = EphemeralTunnelService::default();
         tunnel.revoke_credentials(&token)
     }
     ```
     In the WebAssembly component, `EphemeralTunnelService::default()` is created inside each function invocation. As a consequence, calling `revoke_credentials` on a token minted in a previous call will query a newly initialized, empty `HashMap` and fail with `"Token '...' not found in active credentials registry"`.

2. **`aura-core/src/knowledge/git.rs` PR Creation & Revocation Lifecycle**:
   - `aura-core/src/knowledge/git.rs:270-302`:
     ```rust
     pub fn open_pr(&self, title: &str, _body: &str) -> Result<String, String> {
         let trimmed_title = title.trim();
         if trimmed_title.is_empty() {
             return Err("PR title cannot be empty".to_string());
         }

         // 1. Attest and mint ephemeral child SVID scoped to "github" via IdentityManager
         let identity_manager = IdentityManager::default();
         let parent_svid = Svid::new("spiffe://aura.local/agent/git")
             .with_scopes(vec!["github".to_string(), "tool:ephemeral-tunnel".to_string()]);

         let ephemeral_svid = identity_manager
             .mint_ephemeral_svid(&parent_svid, "pr-creator", &["github".to_string()])
             .map_err(|e| format!("Ephemeral credential minting failed: {:?}", e))?;

         let token = ephemeral_svid.id.clone();
         {
             let mut active = self.active_tokens.lock().unwrap();
             active.insert(token.clone());
         }

         // 2. Perform PR creation using the dynamic ephemeral token
         let pr_result = format!("Opened PR: {}", trimmed_title);

         // 3. Explicit credential revocation immediately upon completion (TOOL-ET-003, IAM-004)
         self.revoke_token(&token)
             .map_err(|e| format!("Credential revocation failed: {}", e))?;

         Ok(pr_result)
     }
     ```
   - `GitManager::open_pr` synthesizes its own `parent_svid` with `"github"` and `"tool:ephemeral-tunnel"` scopes regardless of the caller, granting ambient authority.
   - PR creation is a placeholder string format (`format!("Opened PR: {}", trimmed_title)`), discarding `_body`.
   - If step 2 fails or returns an error, `self.revoke_token(&token)` is skipped, permanently leaving the token in `self.active_tokens`.

3. **`aura-core/src/identity/mod.rs` Workload Attestation & Non-Escalation**:
   - `aura-core/src/identity/mod.rs:567-600`:
     `IdentityManager::mint_ephemeral_svid` checks that all `requested_scopes` are present in `parent_svid.scopes` (unless `parent_svid.is_trusted()`), rejecting with `ScopeDenied` on any attempt to broaden scopes.
   - `Svid` lacks `expires_at`, `issued_at`, and `revoked` fields, and does not provide prompt redaction.

4. **Out-of-Band Sandbox Injection**:
   - `aura-core/src/hypervisor/firecracker.rs:843-850`:
     `BashSessionHandle` possesses `pub current_env: std::collections::HashMap<String, String>`, which represents the execution environment for microVM commands.
   - Neither `tools/ephemeral-tunnel` nor `aura-core` currently provides an out-of-band injection API to inject ephemeral credentials directly into `current_env` or Firecracker microVMs.

5. **Existing Verification Baselines**:
   - `cargo test --manifest-path tools/ephemeral-tunnel/Cargo.toml`: 5 passed; 0 failed.
   - `cargo test -p aura-core --jobs 1 --lib knowledge::git`: 12 passed; 0 failed.
   - `cargo test -p aura-core --jobs 1 --lib identity`: 9 passed; 0 failed.
   - `cargo test -p aura-core --jobs 1 --test m4_empirical_git_stress_test`: 5 passed; 0 failed.
   - Full workspace test: `cargo test -p aura-core --jobs 1`: 146 passed; 0 failed.

---

## 2. Logic Chain

1. **Premise**: Requirement R1 requires that ephemeral credentials "return opaque, sanitized handles (`[REDACTED-TOKEN-...]`) whenever inspected by prompt/logging formatters" and never expose raw secrets to model context or agent memory.
   - **Step 1.1**: Observation 1 shows that while `TokenMetadata::redacted_token` formats the handle correctly, `Credentials` derives `Debug` without masking and does not implement `Display`.
   - **Step 1.2**: Any logging or formatting of `Credentials` directly outputs the raw secret string into logs or context.
   - **Deduction**: `Credentials` must implement custom `std::fmt::Display` and `std::fmt::Debug` that print `[REDACTED-TOKEN-...]` instead of the raw token.

2. **Premise**: Requirement R1 requires that credentials be "automatically revoked upon task completion, cancellation, or failure".
   - **Step 2.1**: Observation 2 shows that in `GitManager::open_pr`, `self.revoke_token(&token)` is executed solely on line 298 after step 2 succeeds.
   - **Step 2.2**: If step 2 encounters an error (e.g. network failure, invalid repository, rejection), `revoke_token` is never reached.
   - **Step 2.3**: Observation 1 shows that in `tools/ephemeral-tunnel/src/lib.rs`, `EphemeralTunnelComponent` instantiates a new `EphemeralTunnelService` on each call, losing track of active tokens across calls.
   - **Deduction**: An RAII drop guard or cleanup mechanism must wrap ephemeral tokens in `open_pr` so revocation executes even on error, and `tools/ephemeral-tunnel/src/lib.rs` must use shared static state.

3. **Premise**: Requirement R1 mandates injecting credentials "out-of-band directly into target execution sandboxes without exposing raw secrets to the model context or agent memory".
   - **Step 3.1**: Observation 4 demonstrates that `BashSessionHandle` has `current_env`, but no out-of-band injection API bridges `EphemeralTunnelService` into `BashSessionHandle` or Firecracker.
   - **Deduction**: An explicit method (e.g., `inject_credentials(&self, handle: &mut BashSessionHandle, creds: &Credentials)`) is required so that secrets enter the sandbox's environment out-of-band while the agent only handles the redacted handle.

4. **Premise**: Requirement R1 mandates "SPIFFE scope authorization checks (caller must be authorized for scope/service)" and "without granting ambient authority".
   - **Step 4.1**: Observation 2 shows that `GitManager::open_pr` currently mints credentials using a hardcoded `parent_svid` with `"github"` and `"tool:ephemeral-tunnel"` scopes, bypassing the calling agent's true identity.
   - **Deduction**: `open_pr` must receive or verify the caller's actual SVID and ensure it holds the `"github"` capability scope before minting ephemeral credentials.

---

## 3. Caveats

- In test/development mode (`AURA_SKIP_KVM_VALIDATION=1`), Firecracker microVM execution falls back to host subshell execution or mocked sockets. The out-of-band injection must be testable under both the real vsock daemon and the host subshell fallback without relying on physical `/dev/kvm`.
- The GitHub REST API / Git CLI network egress proxy allows `api.github.com` and `github.com` in `aura-core/src/hypervisor/proxy.rs:26-28`. In offline test environments, PR creation must support local mock VCS responses or self-contained tests without hitting live external GitHub endpoints.

---

## 4. Conclusion

The existing codebase contains solid building blocks for Phase M5 Requirement R1:
- `tools/ephemeral-tunnel` already provides token minting, verification, TTL checks, and prompt sanitization.
- `aura-core/src/identity` provides Cedar-backed capability enforcement and scope non-escalation.
- `aura-core/src/knowledge/git.rs` has the entry point for `GitManager::open_pr` and token tracking.

To fulfill Requirement R1 completely and satisfy all Phase M5 acceptance criteria, implementation workers must address 4 concrete gaps:
1. **Formatting Redaction**: Implement `Display` and a redacted `Debug` for `Credentials` in `tools/ephemeral-tunnel/src/credentials.rs`.
2. **Component State Sharing**: Store a static shared `EphemeralTunnelService` in `tools/ephemeral-tunnel/src/lib.rs`.
3. **RAII Auto-Revocation on Error**: Add an auto-revoking guard to `GitManager::open_pr` in `aura-core/src/knowledge/git.rs` to guarantee revocation on failure or cancellation.
4. **Out-of-Band Sandbox Injection & Caller Scope Verification**: Wire ephemeral credentials into `BashSessionHandle.current_env` and verify the caller SVID has the `"github"` scope before minting.

The full survey report is available at:
`/home/jeff/aura/.agents/teamwork/teamwork_preview_spec_miner_survey_1/survey_report.md`

---

## 5. Verification Method

To independently verify the observations, claims, and baseline status:

1. **Verify `tools/ephemeral-tunnel` tests**:
   ```bash
   cargo test --manifest-path tools/ephemeral-tunnel/Cargo.toml
   ```
   *Expected output*: 5 passed, 0 failed, 1 doctest passed.

2. **Verify `aura-core` GitManager and Identity tests**:
   ```bash
   cargo test -p aura-core --jobs 1 --lib knowledge::git
   cargo test -p aura-core --jobs 1 --lib identity
   cargo test -p aura-core --jobs 1 --test m4_empirical_git_stress_test
   ```
   *Expected output*: 12 passed in `git.rs`, 9 passed in `identity/mod.rs`, 5 passed in `m4_empirical_git_stress_test.rs`.

3. **Verify Full Baseline Test Suite**:
   ```bash
   cargo test -p aura-core --jobs 1
   ```
   *Expected output*: 126 unit tests, 2 http3 tests, 8 bash tests, 5 cdp tests, 5 git tests (total 146 tests) passing cleanly with 0 failures.

4. **Inspect Source Files**:
   - `tools/ephemeral-tunnel/src/credentials.rs:4-38`
   - `tools/ephemeral-tunnel/src/spire.rs:13-172`
   - `tools/ephemeral-tunnel/src/lib.rs:13-31`
   - `aura-core/src/knowledge/git.rs:270-343`
   - `aura-core/src/identity/mod.rs:567-600`
