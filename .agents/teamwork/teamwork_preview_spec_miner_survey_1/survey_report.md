# Phase M5 Hardening — Requirement R1 (Ephemeral Credential Lifecycle & Out-of-Band Secret Isolation) Specification Mining & Survey Report

- **Surveyor**: `teamwork_preview_spec_miner_survey_1`
- **Date**: 2026-10-11
- **Working Directory**: `/home/jeff/aura`
- **Target Requirement**: Phase M5 (Hardening) § R1 — Ephemeral Credential Lifecycle & Out-of-Band Secret Isolation

---

## 1. Executive Summary

This specification mining report conducts a comprehensive forensic audit of Requirement R1 of Phase M5 (Hardening) for the Aura Multi-Agent Orchestration Operating System.

### Requirement R1 Mandate (from `ORIGINAL_REQUEST.md`):
> **R1. Ephemeral Credential Lifecycle & Out-of-Band Secret Isolation**
> The system must provide an Ephemeral-Tunnel capability that mints short-lived, task-scoped credentials and injects them out-of-band directly into target execution sandboxes without exposing raw secrets to the model context or agent memory. Injected credentials must be automatically revoked upon task completion, cancellation, or failure, and must support end-to-end pull request creation with version control systems without granting ambient authority.

### Key Findings Summary:
1. **Existing Ephemeral Tunnel Infrastructure (`tools/ephemeral-tunnel`)**:
   - `tools/ephemeral-tunnel` is an independent Rust crate (part of the `tools/` workspace) with WIT bindings (`wit/aura-tunnel.wit`).
   - Implements `Credentials`, `TokenMetadata`, and `EphemeralTunnelService`.
   - Supports in-memory minting, allowed service filtering, explicit TTL (default 120s), explicit revocation, verification, and `sanitize_for_prompt()`.
   - **Key Gaps Found**:
     - `Credentials` derives standard `Debug` without custom masking and does not implement `Display`; direct formatting leaks raw tokens!
     - In `tools/ephemeral-tunnel/src/lib.rs`, `EphemeralTunnelComponent` instantiates `EphemeralTunnelService::default()` separately in each function call, resulting in complete state loss between `mint_credentials` and `revoke_credentials`.
     - No out-of-band injection mechanism into sandboxes (Firecracker microVM or WASM) is implemented in `tools/ephemeral-tunnel`.
     - Service authorization uses a static in-memory whitelist rather than integrating with `aura-core`'s Cedar policy engine or SPIFFE SVID verification.

2. **Existing VCS & PR Infrastructure (`aura-core/src/knowledge/git.rs`)**:
   - `GitManager::open_pr` exists and mints an ephemeral child SVID via `IdentityManager::mint_ephemeral_svid`.
   - Maintains `active_tokens` and `revoked_tokens` sets in `GitManager`.
   - **Key Gaps Found**:
     - PR creation logic is purely simulated (`format!("Opened PR: {}", trimmed_title)`). The `_body` parameter is discarded.
     - Token revocation only happens on the happy path. If PR creation errors out or is cancelled, `revoke_token` is never executed, leaking the token in `active_tokens`.
     - `GitManager::open_pr` hardcodes parent identity and scopes (`"github"`, `"tool:ephemeral-tunnel"`), bypassing caller identity verification (ambient authority violation).
     - No integration with `tools/ephemeral-tunnel` or sanitized `[REDACTED-TOKEN-...]` handles.

3. **Existing Identity & Attestation Infrastructure (`aura-core/src/identity/mod.rs`)**:
   - Has `Capability::ToolEphemeralTunnel` (`"tool:ephemeral-tunnel"`).
   - `IdentityManager::mint_ephemeral_svid` enforces non-escalation (requested scopes must be a subset of parent SVID scopes).
   - Cedar policy engine verifies capabilities against Zero-Trust policies.
   - **Key Gaps Found**:
     - `Svid` and `IdentityManager` do NOT track TTL expiration (`expires_at`), do not track revocation status, and do not provide sanitized prompt/logging handles.
     - `aura-core` does not depend on `tools/ephemeral-tunnel`.

---

## 2. Authoritative Specification Sources & Citations

1. **`ORIGINAL_REQUEST.md` (lines 198-245)**:
   - §R1: Short-lived, task-scoped credentials; out-of-band injection into sandboxes; no raw secrets in model context or agent memory (`[REDACTED-TOKEN-...]` handles); automatic revocation on task completion, cancellation, or failure; end-to-end pull request creation via `GitManager::open_pr` without ambient authority.
   - Acceptance Criteria: Sanitized handles upon prompt/logging formatting; authorization denial on unlisted services or unauthorized SPIFFE scopes; out-of-band injection and immediate cross-session revocation; automated PR operations via `GitManager::open_pr` cleanly revoking on completion or error.
2. **`aura-iam-domain` (`.agents/plugins/aura-dev-kit/skills/aura-iam-domain/SKILL.md`)**:
   - Workload identity SVID format: `spiffe://<trust-domain>/<tenant>/<kind>/<name>/<instance>`. Short TTL.
   - Capability calls require SVID with authorized scopes. Non-escalation: scopes never broadened at runtime. Log decisions.
3. **`aura-tool-domain` (`.agents/plugins/aura-dev-kit/skills/aura-tool-domain/SKILL.md`)**:
   - TOOL Domain Rule: "Ephemeral-Tunnel: Mint short-lived credentials; never inject into model context."
4. **`aura-architecture` (`.agents/plugins/aura-dev-kit/skills/aura-architecture/references/03-zero-trust-identity.md`)**:
   - Invariant 2: "Ephemeral credential tools (`Ephemeral-Tunnel`) must communicate directly with the SPIRE server to mint short-lived tokens (e.g., AWS STS, Vault tokens) valid only for the task duration, keeping static secrets completely out of LLM contexts."
5. **Existing Codebase Implementations**:
   - `tools/ephemeral-tunnel/src/credentials.rs:1-38`
   - `tools/ephemeral-tunnel/src/spire.rs:1-173`
   - `tools/ephemeral-tunnel/src/lib.rs:1-35`
   - `tools/ephemeral-tunnel/wit/aura-tunnel.wit:1-23`
   - `aura-core/src/knowledge/git.rs:266-343`
   - `aura-core/src/identity/mod.rs:19-75, 506-628`
   - `aura-core/src/hypervisor/firecracker.rs:842-930`

---

## 3. Features Discovered

| # | Category | Feature | Description | Inputs | Outputs | Error Behavior | Discovered Via |
|---|----------|---------|-------------|--------|---------|----------------|----------------|
| 1 | Ephemeral Credentials | `Credentials` Struct | Struct holding ephemeral credential token and expiration timestamp | `token: String`, `expires_at: u64` | `Credentials` | N/A | `tools/ephemeral-tunnel/src/credentials.rs:5-8` |
| 2 | Ephemeral Credentials | `TokenMetadata` Struct | Full metadata record of an issued token including service, timestamps, revocation state, and scopes | `token`, `service`, `issued_at`, `expires_at`, `revoked`, `scopes` | `TokenMetadata` | N/A | `tools/ephemeral-tunnel/src/credentials.rs:12-19` |
| 3 | Ephemeral Credentials | `TokenMetadata::is_valid` | Validates token is not revoked and current timestamp is strictly before `expires_at` | `current_time: u64` | `bool` | Returns `false` if revoked or expired | `tools/ephemeral-tunnel/src/credentials.rs:23-25` |
| 4 | Ephemeral Credentials | `TokenMetadata::redacted_token` | Formats sanitized representation: `spiffe://aura.local/ephemeral/{service}/[REDACTED-TOKEN-{suffix}]` | `&self` | `String` | Uses `...` suffix if token length < 8 | `tools/ephemeral-tunnel/src/credentials.rs:29-36` |
| 5 | Tunnel Service | `EphemeralTunnelService::new` | Instantiates tunnel service with parent SPIFFE ID, default services (`aws-sts`, `vault`, `spire`, `github`, `database`, `cloud`, `test-service`), and 120s TTL | `parent_spiffe_id: impl Into<String>` | `EphemeralTunnelService` | Fallback default ID `spiffe://aura.local/agent/default` | `tools/ephemeral-tunnel/src/spire.rs:31-47` |
| 6 | Tunnel Service | `with_allowed_services` | Narrows/configures allowed service names for non-escalation enforcement | `services: Vec<String>` | `EphemeralTunnelService` | Overwrites `allowed_services` set | `tools/ephemeral-tunnel/src/spire.rs:50-53` |
| 7 | Tunnel Service | `with_ttl_secs` | Configures explicit TTL duration in seconds for minted credentials | `ttl: u64` | `EphemeralTunnelService` | Sets `default_ttl_secs` | `tools/ephemeral-tunnel/src/spire.rs:56-59` |
| 8 | Tunnel Service | `mint_credentials` | Mints short-lived token (`spiffe://aura.local/ephemeral/{service}/{uuid}`), registers in thread-safe map, returns `Credentials` | `service: &str` | `Result<Credentials, String>` | Errors on empty service or if service not in `allowed_services` | `tools/ephemeral-tunnel/src/spire.rs:71-111` |
| 9 | Tunnel Service | `revoke_credentials` | Explicitly invalidates active token by setting `revoked = true` in registry | `token: &str` | `Result<(), String>` | Errors if already revoked or token not found | `tools/ephemeral-tunnel/src/spire.rs:114-133` |
| 10 | Tunnel Service | `verify_token` | Verifies token exists, is not revoked, and is within valid TTL | `token: &str` | `Result<TokenMetadata, String>` | Errors on not found, revoked, or expired | `tools/ephemeral-tunnel/src/spire.rs:136-159` |
| 11 | Tunnel Service | `sanitize_for_prompt` | Returns sanitized handle for prompt/logging inspection without leaking raw secret | `token: &str` | `String` | Returns `[REDACTED-TOKEN]` if token unknown or lock fails | `tools/ephemeral-tunnel/src/spire.rs:162-171` |
| 12 | WIT Interface | `aura:tunnel@1.0.0` WIT contract | WebAssembly Component Model WIT interface definition for `ephemeral` world | `service: string`, `token: string` | `credentials`, `result` | Returns WIT `result<_, string>` | `tools/ephemeral-tunnel/wit/aura-tunnel.wit:1-23` |
| 13 | WIT Component | `EphemeralTunnelComponent` | Component export implementation for `aura:tunnel` | `service: String`, `token: String` | `Credentials`, `()` | Propagates error strings | `tools/ephemeral-tunnel/src/lib.rs:13-31` |
| 14 | Git PR Creation | `GitManager::open_pr` | Mints ephemeral SVID for `"github"`, executes PR, and revokes token | `title: &str`, `_body: &str` | `Result<String, String>` | Errors on empty title, minting failure, or revocation failure | `aura-core/src/knowledge/git.rs:270-302` |
| 15 | Git Token Mgmt | `GitManager::revoke_token` | Removes token from `active_tokens` and adds to `revoked_tokens` | `token: &str` | `Result<(), String>` | Returns Err on lock acquisition failure | `aura-core/src/knowledge/git.rs:305-318` |
| 16 | Git Token Mgmt | `GitManager::is_token_active` | Checks whether token is currently tracked in `active_tokens` | `token: &str` | `bool` | Returns `false` on lock error | `aura-core/src/knowledge/git.rs:329-334` |
| 17 | Git Token Mgmt | `GitManager::is_token_revoked` | Checks whether token is tracked in `revoked_tokens` | `token: &str` | `bool` | Returns `false` on lock error | `aura-core/src/knowledge/git.rs:321-326` |
| 18 | IAM Attestation | `IdentityManager::mint_ephemeral_svid` | Mints child SVID `{parent_id}/ephemeral/{agent_name}/{uuid}` with scope non-escalation check | `parent_svid: &Svid`, `agent_name: &str`, `requested_scopes: &[String]` | `Result<Svid, ScopeDenied>` | Returns `ScopeDenied` if requested scope not in parent scopes | `aura-core/src/identity/mod.rs:567-600` |
| 19 | IAM Capabilities | `Capability::ToolEphemeralTunnel` | Strongly-typed capability enum variant for `"tool:ephemeral-tunnel"` | String `"tool:ephemeral-tunnel"` | `Capability` | N/A | `aura-core/src/identity/mod.rs:30, 45, 64` |
| 20 | IAM Policy | `CapabilityEnforcer::check_scope` | Cedar policy and SVID scope verification for granular capability authorization | `svid: &Svid`, `scope: &str` | `Result<(), ScopeDenied>` | Returns `ScopeDenied` (403) on unauthorized scope | `aura-core/src/identity/mod.rs:338-362` |
| 21 | MicroVM Session | `BashSessionHandle::current_env` | Environment variable map stored in persistent microVM shell session handle | `HashMap<String, String>` | Environment state | N/A | `aura-core/src/hypervisor/firecracker.rs:849` |
| 22 | Tool Manifest | `tools/ephemeral-tunnel.json` | Declarative tool manifest declaring WASI 0.3 execution environment and WASM memory isolation | Manifest JSON | Tool definition | N/A | `tools/ephemeral-tunnel.json:1-7` |
| 23 | Python SDK Tool | `EphemeralTunnelTool.mint_svid` | Python SDK wrapper dispatching `"ephemeral-tunnel"` tool over Zenoh client bus | `agent_name: str` | `Coroutine[str]` | Propagates Zenoh transport error | `aura-sdk/aura_sdk/tools/ephemeral_tunnel.py:4-9` |

---

## 4. Edge Cases Probed & Observed Behaviors

| # | Feature | Input | Observed Behavior |
|---|---------|-------|-------------------|
| 1 | `EphemeralTunnelService::mint_credentials` | Empty service name `""` or `"   "` | Returns `Err("Service name cannot be empty")`. Validated in code (`spire.rs:73`). |
| 2 | `EphemeralTunnelService::mint_credentials` | Unauthorized service (e.g. `"aws-sts"` on restricted instance) | Returns `Err("Privilege escalation denied: service 'aws-sts' is not authorized under parent identity ...")`. Tested in `tests/tunnel_test.rs:18-35`. |
| 3 | `EphemeralTunnelService::mint_credentials` | Service with leading/trailing spaces (e.g. `" github "`) | Trimmed successfully to `"github"`, proceeds with authorization check. |
| 4 | `EphemeralTunnelService::revoke_credentials` | Non-existent token `spiffe://aura.local/ephemeral/test/nonexistent` | Returns `Err("Token '...' not found in active credentials registry")`. |
| 5 | `EphemeralTunnelService::revoke_credentials` | Token that has already been revoked | Returns `Err("Token '...' has already been revoked")`. Tested in `tests/tunnel_test.rs:66-68`. |
| 6 | `EphemeralTunnelService::verify_token` | Token after explicit revocation | Returns `Err("Token '...' is revoked")`. Tested in `tests/tunnel_test.rs:61-63`. |
| 7 | `EphemeralTunnelService::verify_token` | Token with `current_time >= expires_at` | Returns `Err("Token '...' expired at ... (current time: ...)")`. |
| 8 | `EphemeralTunnelService::sanitize_for_prompt` | Non-existent or unrecognized token | Returns `"[REDACTED-TOKEN]"`. Tested in `spire.rs:170`. |
| 9 | `EphemeralTunnelService::sanitize_for_prompt` | Valid token with length >= 8 | Returns `spiffe://aura.local/ephemeral/{service}/[REDACTED-TOKEN-{last_8_chars}]`. Tested in `tests/tunnel_test.rs:38-47`. |
| 10 | `Credentials` Debug Formatting | `format!("{:?}", creds)` | **Vulnerability**: Prints `Credentials { token: "spiffe://aura.local/ephemeral/...", expires_at: ... }`, leaking the raw secret! Missing custom `Debug` / `Display` redaction. |
| 11 | `Credentials` Display Formatting | `format!("{}", creds)` | Fails to compile because `std::fmt::Display` is not implemented for `Credentials`. |
| 12 | `EphemeralTunnelComponent` in `lib.rs` | Consecutive call: `mint_credentials("github")` then `revoke_credentials(token)` | **Defect**: Returns `Err("Token '...' not found in active credentials registry")` because `lib.rs:19` and `lib.rs:28` each instantiate a brand new `EphemeralTunnelService::default()`. State is NOT shared across component invocations. |
| 13 | `GitManager::open_pr` | Empty or whitespace PR title (`"   "`) | Returns `Err("PR title cannot be empty")`. Tested in `knowledge/git.rs:513-518`. |
| 14 | `GitManager::open_pr` | PR creation error simulation (if PR logic fails) | **Defect**: `revoke_token(&token)` is placed at the end of the method; if PR creation fails or returns early, the minted token remains leaked in `active_tokens` permanently. |
| 15 | `GitManager::open_pr` | Concurrent multithreaded PR operations (12 threads x 50 iterations) | Thread-safe locks prevent data races, all tokens cleanly revoked on happy path. Tested in `tests/m4_empirical_git_stress_test.rs:215-275`. |
| 16 | `IdentityManager::mint_ephemeral_svid` | Caller requests scope not in parent SVID (e.g. `fs:write` when parent has only `fs:read`) | Returns `Err(ScopeDenied)`: `Zero-Trust Violation: SVID ... is not authorized for capability 'fs:write' by Cedar policy (403 ScopeDenied: Cannot broaden scopes at runtime...)`. Tested in `identity/tests:766-784`. |
| 17 | `IdentityManager::mint_ephemeral_svid` | Empty requested scopes `&[]` | Defaults to cloning all parent SVID scopes (`parent_svid.scopes.clone()`). |
| 18 | `IdentityManager::mint_ephemeral_svid` | Trusted/host parent identity (`spiffe://aura.local/host`) | Bypasses scope check because `parent_svid.is_trusted() == true`, permits any requested scope. |
| 19 | Out-of-band Credential Injection | Injecting minted token into sandbox | **Missing Feature**: Neither `tools/ephemeral-tunnel` nor `GitManager` currently injects tokens out-of-band into microVM environments (`BashSessionHandle.current_env` or Firecracker vsock). |

---

## 5. Architectural Dissection & Existing Codebase Structure

### 5.1 `tools/ephemeral-tunnel` Structure
```
tools/ephemeral-tunnel/
├── Cargo.toml          (Defines package ephemeral-tunnel, cdylib + rlib, deps: wit-bindgen, serde, serde_json, uuid)
├── src/
│   ├── lib.rs          (Component bindings, exports::aura::tunnel::ephemeral::Guest)
│   ├── credentials.rs  (Credentials, TokenMetadata, is_valid, redacted_token)
│   └── spire.rs        (EphemeralTunnelService, mint_credentials, revoke_credentials, verify_token, sanitize_for_prompt)
├── tests/
│   └── tunnel_test.rs  (5 tests: mint, non-escalation, sanitization, revocation, submillisecond performance)
└── wit/
    └── aura-tunnel.wit (WIT definition for package aura:tunnel@1.0.0, interface ephemeral, worlds tunnel/tunnel-service)
```

#### Detailed Source Analysis:
- **`credentials.rs` (lines 4-37)**:
  - `Credentials`:
    ```rust
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct Credentials {
        pub token: String,
        pub expires_at: u64,
    }
    ```
    *Issue*: `Debug` is derived, exposing `token` in plaintext.
  - `TokenMetadata`:
    Contains `redacted_token()`:
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
- **`spire.rs` (lines 13-172)**:
  - `EphemeralTunnelService` stores `tokens: Arc<Mutex<HashMap<String, TokenMetadata>>>`.
  - Default TTL: `DEFAULT_CREDENTIAL_TTL_SECS = 120`.
  - Token format generated: `spiffe://aura.local/ephemeral/{service}/{uuid}`.
  - Verification checks: `meta.revoked` and `current_timestamp >= meta.expires_at`.
- **`lib.rs` (lines 13-31)**:
  - Implements WIT `Guest` trait.
  - *Critical Defect*: Functions instantiate `EphemeralTunnelService::default()` on the fly:
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
    This means the token state is discarded immediately upon return, making component-level revocation impossible. Needs a static shared `Lazy<EphemeralTunnelService>` or `OnceLock`.

### 5.2 `aura-core/src/knowledge/git.rs` Structure
- **`GitManager` fields**:
  - `pub active_tokens: Arc<Mutex<HashSet<String>>>`
  - `pub revoked_tokens: Arc<Mutex<HashSet<String>>>`
- **`GitManager::open_pr` (lines 270-302)**:
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
  *Analysis of Deficiencies*:
  1. No out-of-band injection into sandbox: the token is never passed to an execution environment.
  2. The parent SVID is synthesized within `open_pr` with ambient authority (`"github"` and `"tool:ephemeral-tunnel"` scopes hardcoded).
  3. No RAII guard: if step 2 errors or panics, the token remains in `active_tokens`.
  4. Returns a plain string without returning an opaque sanitized handle `[REDACTED-TOKEN-...]`.

### 5.3 `aura-core/src/identity/mod.rs` Structure
- **`Svid` (lines 157-205)**:
  - Fields: `id: String`, `trust_domain: String`, `scopes: HashSet<String>`, `allowed_paths: Vec<PathBuf>`, `cert_chain_der`, `private_key_der`, `hint`.
  - Lacks `expires_at`, `issued_at`, and `revoked` fields.
- **`IdentityManager::mint_ephemeral_svid` (lines 567-600)**:
  - Enforces scope non-escalation: requested scopes must be subset of `parent_svid.scopes`.
  - Generates SVID string: `{parent_svid.id}/ephemeral/{agent_name}/{uuid}`.
  - Does not manage TTL or revocation lifecycle.

### 5.4 `aura-core/src/runtime/registry.rs` Structure
- Tool manifest schema: `ToolManifestFile` with `ToolEntryConfig::Wasm` and `ToolEntryConfig::Firecracker`.
- `Registry::register_default_sandboxes` registers `"bash"`, `"browser"`, `"computer"`.
- Does not register `"ephemeral-tunnel"`.

---

## 6. Gap Analysis Matrix: What is Implemented vs. What is Missing for R1

| Requirement R1 Component | Current Implementation Status | What Exists in Codebase | What is Missing / Defective |
|---|---|---|---|
| **1. Explicit TTL & Sanitized `[REDACTED-TOKEN-...]` Handles** | **Partial** | `TokenMetadata::redacted_token` produces `spiffe://aura.local/ephemeral/{service}/[REDACTED-TOKEN-{suffix}]`. `EphemeralTunnelService` has `with_ttl_secs(ttl)`. | `Credentials` lacks custom `Display`/`Debug` masking (printing leaks raw token). `Svid` in `aura-core` has no TTL or redaction. No prompt/logging formatter hook in `aura-core`. |
| **2. SPIFFE Scope Authorization Checks** | **Partial** | `EphemeralTunnelService` checks `allowed_services.contains(service)`. `IdentityManager::mint_ephemeral_svid` checks `parent_svid.scopes.contains(req)`. | In `ephemeral-tunnel`, caller is just a string without cryptographic SVID attestation. In `GitManager::open_pr`, parent SVID is hardcoded with ambient permissions instead of checking the calling agent's attested SVID. |
| **3. Out-of-Band Sandbox Injection** | **Missing** | `BashSessionHandle.current_env` exists in `firecracker.rs`. | Neither `tools/ephemeral-tunnel` nor `aura-core` has an out-of-band credential injection mechanism that injects tokens directly into guest sandboxes while keeping them out of agent context. |
| **4. Automatic Revocation (Completion, Cancellation, Failure)** | **Partial** | `EphemeralTunnelService::revoke_credentials` and `GitManager::revoke_token` exist for explicit revocation. | No automatic RAII cleanup on error/failure in `GitManager::open_pr`. No hook in Zenoh bus `tasks/completion` to automatically revoke task credentials. WIT component resets state on every call. |
| **5. End-to-End PR Creation in `GitManager::open_pr`** | **Partial** | Method signature exists, mints SVID, revokes on happy path. | Simulated string output (`"Opened PR: ..."`). Discards `_body`. Does not inject token into Git command/env. Does not revoke on error. |

---

## 7. Detailed Interface Contracts & File Boundaries

### 7.1 Existing Type Signatures

#### `tools/ephemeral-tunnel/src/credentials.rs`
```rust
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credentials {
    pub token: String,
    pub expires_at: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenMetadata {
    pub token: String,
    pub service: String,
    pub issued_at: u64,
    pub expires_at: u64,
    pub revoked: bool,
    pub scopes: Vec<String>,
}

impl TokenMetadata {
    pub fn is_valid(&self, current_time: u64) -> bool;
    pub fn redacted_token(&self) -> String;
}
```

#### `tools/ephemeral-tunnel/src/spire.rs`
```rust
pub const DEFAULT_CREDENTIAL_TTL_SECS: u64 = 120;

#[derive(Clone, Debug)]
pub struct EphemeralTunnelService {
    parent_spiffe_id: String,
    allowed_services: HashSet<String>,
    default_ttl_secs: u64,
    tokens: Arc<Mutex<HashMap<String, TokenMetadata>>>,
}

impl EphemeralTunnelService {
    pub fn new(parent_spiffe_id: impl Into<String>) -> Self;
    pub fn with_allowed_services(mut self, services: Vec<String>) -> Self;
    pub fn with_ttl_secs(mut self, ttl: u64) -> Self;
    pub fn mint_credentials(&self, service: &str) -> Result<Credentials, String>;
    pub fn revoke_credentials(&self, token: &str) -> Result<(), String>;
    pub fn verify_token(&self, token: &str) -> Result<TokenMetadata, String>;
    pub fn sanitize_for_prompt(&self, token: &str) -> String;
}
```

#### `aura-core/src/knowledge/git.rs`
```rust
pub struct GitManager {
    pub current_branch: String,
    pub tracked_files: Arc<Mutex<Vec<PathBuf>>>,
    pub merkle_tree: Arc<Mutex<Option<MerkleTree>>>,
    pub authored_files: Arc<Mutex<HashSet<PathBuf>>>,
    pub vm_config: Arc<Mutex<Option<VmConfig>>>,
    pub linter_error: Arc<Mutex<Option<String>>>,
    pub active_tokens: Arc<Mutex<HashSet<String>>>,
    pub revoked_tokens: Arc<Mutex<HashSet<String>>>,
    pub patch_history: Arc<Mutex<Vec<String>>>,
}

impl GitManager {
    pub fn new(branch: impl Into<String>) -> Self;
    pub fn open_pr(&self, title: &str, body: &str) -> Result<String, String>;
    pub fn revoke_token(&self, token: &str) -> Result<(), String>;
    pub fn is_token_revoked(&self, token: &str) -> bool;
    pub fn is_token_active(&self, token: &str) -> bool;
}
```

#### `aura-core/src/identity/mod.rs`
```rust
pub enum Capability {
    ...
    ToolEphemeralTunnel,
    Custom(String),
}

pub struct ScopeDenied {
    pub svid: String,
    pub requested_scope: String,
    pub reason: String,
}

pub struct Svid {
    pub id: String,
    pub trust_domain: String,
    pub scopes: HashSet<String>,
    pub allowed_paths: Vec<PathBuf>,
    pub cert_chain_der: Vec<Vec<u8>>,
    pub private_key_der: Option<Vec<u8>>,
    pub hint: Option<String>,
}

impl IdentityManager {
    pub fn mint_ephemeral_svid(
        &self,
        parent_svid: &Svid,
        agent_name: &str,
        requested_scopes: &[String],
    ) -> Result<Svid, ScopeDenied>;
}
```

### 7.2 File Boundaries & Ownership for Phase M5 R1

1. **`tools/ephemeral-tunnel/src/credentials.rs`**:
   - Implement custom `std::fmt::Display` and `std::fmt::Debug` for `Credentials` to always display `[REDACTED-TOKEN-...]` opaque handles.
   - Add explicit `redacted_handle(&self) -> String` to `Credentials`.
2. **`tools/ephemeral-tunnel/src/lib.rs`**:
   - Fix component state persistence: replace per-call `EphemeralTunnelService::default()` with shared singleton state (`std::sync::OnceLock` or `lazy_static`/`Arc`).
3. **`tools/ephemeral-tunnel/src/spire.rs`**:
   - Enhance scope validation and service authorization.
   - Add out-of-band injection helper or sandbox environment payload preparation.
4. **`aura-core/src/knowledge/git.rs`**:
   - Refactor `GitManager::open_pr` to:
     - Enforce caller SVID authorization (verify caller has `"github"` scope).
     - Implement RAII auto-revocation guard (ensuring tokens are revoked even on errors or panics).
     - Support authentic VCS integration or out-of-band injection into sandbox.
     - Return sanitized handle or verify prompt formatting sanitization.
5. **`aura-core/src/identity/mod.rs`**:
   - Optionally integrate TTL and active token tracking into `IdentityManager` or link `ephemeral-tunnel` crate.
6. **Integration Tests**:
   - `tools/ephemeral-tunnel/tests/tunnel_test.rs`: Add tests verifying `Display`/`Debug` redaction on `Credentials`.
   - `aura-core/tests/m4_empirical_git_stress_test.rs`: Add tests for `open_pr` revocation on simulated error/failure.

---

## 8. Verification Matrix & Concrete Recommendations

### Verification Commands:
- `cargo test --manifest-path tools/ephemeral-tunnel/Cargo.toml` (currently 5 passing tests, 1 doctest).
- `cargo test -p aura-core --jobs 1 --lib knowledge::git` (currently 12 passing tests).
- `cargo test -p aura-core --jobs 1 --lib identity` (currently 9 passing tests).
- `cargo test -p aura-core --jobs 1 --test m4_empirical_git_stress_test` (currently 5 passing tests).
- Full suite: `cargo test -p aura-core --jobs 1` (currently 146 passing tests).

### Recommended Implementation Steps for M5 Workers:
1. **Redact Credentials Formatting (`tools/ephemeral-tunnel/src/credentials.rs`)**:
   Implement `Display` and a custom `Debug` for `Credentials` so that `format!("{}", creds)` and `format!("{:?}", creds)` always output `[REDACTED-TOKEN-...]` instead of the raw secret token.
2. **Fix WIT Component State (`tools/ephemeral-tunnel/src/lib.rs`)**:
   Introduce a static global `Arc<EphemeralTunnelService>` so that `revoke_credentials` called over WASM correctly accesses the tokens minted by `mint_credentials`.
3. **RAII Credential Revocation Guard in `GitManager::open_pr` (`aura-core/src/knowledge/git.rs`)**:
   Wrap the active token in an RAII drop guard (or a `scopeguard`-style pattern) so that regardless of whether PR creation succeeds, fails, or returns early, `revoke_token(&token)` is guaranteed to run.
4. **Out-of-Band Sandbox Injection Contract**:
   Create a direct mechanism to inject credentials into execution sandbox sessions (such as `BashSessionHandle.current_env` in `firecracker.rs`) without exposing raw secrets to the agent conversation context.
5. **Caller Scope Authorization in `open_pr`**:
   Ensure `open_pr` accepts or checks the caller's actual SVID rather than synthesizing an ambient root SVID.
