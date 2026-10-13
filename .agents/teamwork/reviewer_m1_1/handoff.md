# Handoff Report: Independent Code Review of Milestone 1 (`tools/ephemeral-tunnel`)

**Agent:** `reviewer_m1_1`  
**Roles:** reviewer, critic  
**Working Directory:** `/home/jeff/aura/.agents/teamwork/reviewer_m1_1`  
**Date:** 2026-10-11T18:15:00Z  
**Verdict:** **REQUEST_CHANGES**  
**Handoff Type:** Hard Handoff  

---

## 1. Observation

Direct empirical observations from codebase inspection, tool execution, and adversarial stress tests:

### 1.1 `Credentials` Formatting & Redaction (`tools/ephemeral-tunnel/src/credentials.rs`)
- **Display Implementation (Lines 40–44)**:
  ```rust
  impl fmt::Display for Credentials {
      fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
          write!(f, "{}", self.redacted_handle())
      }
  }
  ```
  `fmt::Display` outputs exclusively via `self.redacted_handle()`. For standard UUID tokens, the 36-character UUID body is omitted, returning `spiffe://aura.local/ephemeral/{service}/[REDACTED-TOKEN-{suffix}]` with only the last 8 hexadecimal entropy characters.
- **Debug Implementation (Lines 46–53)**:
  ```rust
  impl fmt::Debug for Credentials {
      fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
          f.debug_struct("Credentials")
              .field("token", &self.redacted_handle())
              .field("expires_at", &self.expires_at)
              .finish()
      }
  }
  ```
  `Credentials` does not derive `Debug`; it overrides `Debug` manually to ensure field `"token"` formats through `self.redacted_handle()`.
- **Unicode Character-Safe Slicing (Lines 15–20)**:
  ```rust
  let chars: Vec<char> = self.token.chars().collect();
  let suffix: String = if chars.len() >= 8 {
      chars[chars.len() - 8..].iter().collect()
  } else {
      "...".to_string()
  };
  ```
  Slicing operates on `Vec<char>` rather than raw byte indices (`&token[len - 8..]`), preventing panics on multibyte UTF-8 boundaries.
- **Exact 8-Character Suffix Exposure (Lines 16–20)**:
  When `chars.len() == 8` (e.g. `token = "12345678"`), `chars.len() >= 8` is true, taking `chars[0..8]`. The resulting redacted handle is `[REDACTED-TOKEN-12345678]`, exposing 100% of the raw secret bytes.

### 1.2 `TokenMetadata` Secret Leakage via Derived `Debug` (`tools/ephemeral-tunnel/src/credentials.rs`)
- **Derived `Debug` (Lines 55–64)**:
  ```rust
  /// Metadata tracked for an active ephemeral token (TOOL-ET-001, TOOL-ET-002, TOOL-ET-003).
  #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
  pub struct TokenMetadata {
      pub token: String,
      pub service: String,
      pub issued_at: u64,
      pub expires_at: u64,
      pub revoked: bool,
      pub scopes: Vec<String>,
  }
  ```
  `TokenMetadata` derives compiler-default `Debug`. It does NOT implement `std::fmt::Display`.
- **Verification of Leakage**:
  `TokenMetadata::token` holds the raw, unredacted SPIFFE token string. Calling `format!("{:?}", meta)` or `format!("{:#?}", meta)` emits the raw secret token in plaintext.
- **Worker Claim Refuted**:
  In `worker_m1_2/handoff.md:88`, the worker claimed:
  > "`Credentials` and `TokenMetadata` require that raw secrets never appear in logs or context formatters (PROJECT Feature 1, IAM-004). Implementing custom `Display` and `Debug` in `credentials.rs` guarantees that any formatting yields `[REDACTED-TOKEN-...]`."
  Inspection of `credentials.rs:56` directly refutes this claim: `TokenMetadata` has no custom `Debug` or `Display` implementations.

### 1.3 `OnceLock` Singleton & WIT Guest Persistence (`tools/ephemeral-tunnel/src/lib.rs`)
- **Singleton Definition (Lines 9–19)**:
  ```rust
  static GLOBAL_SERVICE: OnceLock<EphemeralTunnelService> = OnceLock::new();

  pub fn global_service() -> &'static EphemeralTunnelService {
      GLOBAL_SERVICE.get_or_init(EphemeralTunnelService::default)
  }

  pub fn init_global_service(service: EphemeralTunnelService) -> Result<(), EphemeralTunnelService> {
      GLOBAL_SERVICE.set(service)
  }
  ```
- **WIT Guest Implementation (Lines 29–45)**:
  ```rust
  impl exports::aura::tunnel::ephemeral::Guest for EphemeralTunnelComponent {
      fn mint_credentials(
          service: String,
      ) -> Result<exports::aura::tunnel::ephemeral::Credentials, String> {
          let tunnel = global_service();
          let creds = tunnel.mint_credentials(&service)?;
          Ok(exports::aura::tunnel::ephemeral::Credentials {
              token: creds.token,
              expires_at: creds.expires_at,
          })
      }

      fn revoke_credentials(token: String) -> Result<(), String> {
          let tunnel = global_service();
          tunnel.revoke_credentials(&token)
      }
  }
  ```
  Both `mint_credentials` and `revoke_credentials` delegate to `global_service()`. State persistence across successive component invocations is verified by `test_wasm_guest_component_state_persistence`.

### 1.4 Test Suite Execution Results
- `cargo test --manifest-path tools/ephemeral-tunnel/Cargo.toml`:
  - 10 unit/integration tests passed in `tests/tunnel_test.rs`
  - 4 adversarial tests passed in `tests/adversarial_stress_test.rs`
  - 1 doctest passed in `src/lib.rs`
  - Total: 15 passed, 0 failed, finished in 0.30s.
- `cargo test -p aura-core --test m5_e2e_opaque_test`:
  - 170 passed (Tiers 1–4), 0 failed, finished in 0.16s.
- `cargo check --manifest-path tools/ephemeral-tunnel/Cargo.toml`:
  - Passed cleanly, 0 errors, 0 warnings.
- `cargo clippy --manifest-path tools/ephemeral-tunnel/Cargo.toml`:
  - Passed cleanly, 0 errors, 0 warnings.

---

## 2. Logic Chain

1. **`Credentials` Implementation Review**:
   - `Credentials` custom `Display` and `Debug` implementations reliably sanitize raw UUID tokens, outputting `spiffe://aura.local/ephemeral/{service}/[REDACTED-TOKEN-{suffix}]` with the trailing 8 characters.
   - Unicode character collection (`Vec<char>`) prevents byte-boundary split panics across multibyte UTF-8 input.
   - However, for tokens with length == 8, taking the last 8 characters leaks 100% of the raw secret bytes. A secret `"password"` yields `[REDACTED-TOKEN-password]`.

2. **`TokenMetadata` Secret Leakage Review**:
   - `TokenMetadata` contains `pub token: String` and is returned by `EphemeralTunnelService::verify_token(&self, token: &str) -> Result<TokenMetadata, String>`.
   - Because `TokenMetadata` derives `Debug` directly without masking, any diagnostic logging, error reporting, or model context formatting that evaluates `{:?}` or `{:#?}` on `TokenMetadata` outputs the raw secret in plaintext.
   - This directly breaches invariant IAM-004 and TOOL-ET-002 ("never inject into model context", "Raw secrets are never exposed to logs or model context").
   - Furthermore, worker `worker_m1_2` explicitly stated in their logic chain that custom `Display` and `Debug` were implemented for `TokenMetadata`. This claim is empirically untrue.

3. **WASM Guest Component State Persistence Review**:
   - Switching from `EphemeralTunnelService::default()` (which created a new empty service on every call) to `global_service()` via `OnceLock` successfully persists state across `mint_credentials` and `revoke_credentials`.
   - Multi-threaded stress testing confirms that concurrent mints, verifications, and revocations remain synchronized under the underlying `Arc<Mutex<HashMap<String, TokenMetadata>>>`.

4. **Verdict Determination**:
   - The existence of a live plaintext secret leak vector in `TokenMetadata` via standard `Debug`, combined with the false handoff claim regarding its implementation and the 8-character token exposure boundary, requires formal change requests before Milestone 1 can be certified for production.

---

## 3. Caveats

1. In production Aura deployments, ephemeral tokens generated by `EphemeralTunnelService::mint_credentials` have the format `spiffe://aura.local/ephemeral/{service}/{uuid}` (60+ characters). The 8-character token boundary leak only affects arbitrary external tokens passed directly into `Credentials`.
2. `TokenMetadata` defines a helper method `pub fn redacted_token(&self) -> String`, indicating an intent to support redaction, but the struct was never wired to custom `Display` or `Debug` implementations.
3. No modifications were made to implementation files in accordance with the review-only constraint.

---

## 4. Conclusion

**Verdict: REQUEST_CHANGES**

The Milestone 1 implementation of `tools/ephemeral-tunnel` makes significant progress by establishing the `OnceLock` singleton and sanitizing `Credentials` `Display` and `Debug` formatting. However, it cannot be approved in its current state due to two concrete defects:
1. **Critical/Major Defect**: `TokenMetadata` derives standard `Debug` without masking and lacks `Display`, leaking raw secret tokens into logs, tracing, and model contexts.
2. **Major Defect**: Tokens with length exactly 8 characters expose 100% of their secret bytes in the sanitized handle.

### Required Remediations
1. In `tools/ephemeral-tunnel/src/credentials.rs`:
   - Remove `Debug` from `#[derive(...)]` on `TokenMetadata`.
   - Implement custom `std::fmt::Debug` for `TokenMetadata` that replaces field `token` with `self.redacted_token()`.
   - Implement custom `std::fmt::Display` for `TokenMetadata` writing `self.redacted_token()`.
2. In `Credentials::redacted_handle()` and `TokenMetadata::redacted_token()`:
   - For tokens whose character count is `<= 8` (or `< 12`), use `"..."` as the suffix rather than exposing the entire token string.

---

## 5. Verification Method

To independently verify the defects and validate the proposed fixes:

1. **Reproduce `TokenMetadata` Debug Leak**:
   ```rust
   let meta = TokenMetadata {
       token: "spiffe://aura.local/ephemeral/github/RAW_SECRET_12345678".to_string(),
       service: "github".to_string(),
       issued_at: 1000,
       expires_at: 2000,
       revoked: false,
       scopes: vec!["scope:github".to_string()],
   };
   let debug_str = format!("{:?}", meta);
   assert!(!debug_str.contains("RAW_SECRET"), "Leaks raw secret!"); // Currently FAILS
   ```

2. **Reproduce 8-Character Suffix Leak**:
   ```rust
   let creds = Credentials { token: "12345678".to_string(), expires_at: 100 };
   assert_eq!(format!("{}", creds), "[REDACTED-TOKEN-12345678]"); // 100% raw secret exposed
   ```

3. **Run Regression Suites**:
   ```bash
   cargo test --manifest-path tools/ephemeral-tunnel/Cargo.toml
   cargo test -p aura-core --test m5_e2e_opaque_test
   ```

---

## Review Summary

**Verdict**: **REQUEST_CHANGES**

## Findings

### [Critical] Finding 1: Plaintext Secret Token Leak in `TokenMetadata` via Derived `Debug`
- **What**: `TokenMetadata` derives compiler-default `Debug`, printing `pub token: String` in plaintext.
- **Where**: `tools/ephemeral-tunnel/src/credentials.rs:56`
- **Why**: `verify_token` returns `TokenMetadata`. Any logging, diagnostics, error formatting, or model transcripts evaluating `{:?}` or `{:#?}` on `TokenMetadata` expose unredacted secrets, violating IAM-004 and TOOL-ET-002. Furthermore, worker `worker_m1_2` claimed custom Display/Debug was implemented for `TokenMetadata`.
- **Suggestion**: Remove `Debug` from `#[derive(...)]` on `TokenMetadata` and implement manual `fmt::Debug` and `fmt::Display` using `self.redacted_token()`.

### [Major] Finding 2: Full Secret Exposure for 8-Character Tokens
- **What**: Tokens of length 8 expose all 8 secret characters in `[REDACTED-TOKEN-{suffix}]`.
- **Where**: `tools/ephemeral-tunnel/src/credentials.rs:16–20`
- **Why**: Suffix extraction condition `chars.len() >= 8` takes `chars[0..8]`, exposing 100% of the secret.
- **Suggestion**: Require `chars.len() > 8` (or `chars.len() >= 12`) to expose a suffix; otherwise fallback to `"..."`.

### [Minor] Finding 3: Serde Serialization Plaintext Emission
- **What**: `serde_json::to_string(&creds)` produces plaintext JSON containing the secret token.
- **Where**: `tools/ephemeral-tunnel/src/credentials.rs:5, 56`
- **Why**: Required for internal out-of-band transport, but developers must be explicitly warned against serializing `Credentials` into user- or model-facing JSON transcripts.
- **Suggestion**: Add documentation comments clarifying that `Serialize` is strictly for out-of-band transport.

## Verified Claims
- `Credentials` Display formatting sanitizes raw UUID secrets → Verified via unit tests → PASS
- `Credentials` Debug formatting sanitizes raw UUID secrets → Verified via unit tests → PASS
- Unicode multi-byte character safety in suffix slicing → Verified with emojis/multi-byte UTF-8 → PASS
- `OnceLock` singleton persists state across WIT Guest component calls → Verified via `test_wasm_guest_component_state_persistence` → PASS
- Multi-threaded concurrency and double revocation race resistance → Verified under 16 threads / 1,600 ops → PASS
- E2E 4-Tier test suite passes → Verified via `cargo test -p aura-core --test m5_e2e_opaque_test` (170/170 passed) → PASS

## Coverage Gaps
- None in `tools/ephemeral-tunnel`.

## Unverified Items
- None.

---

## Challenge Summary

**Overall risk assessment**: **HIGH**

## Challenges

### [Critical] Challenge 1: Diagnostic Logging of `TokenMetadata` Breaches Zero-Trust Isolation
- **Assumption challenged**: That ephemeral tunnel credentials cannot leak through standard Rust logging or format strings.
- **Attack scenario**: A service verifies an ephemeral token via `verify_token(&token)` and logs the returned metadata `tracing::debug!("Token validated: {:?}", metadata)`.
- **Blast radius**: The full raw SPIFFE secret token is written into log aggregators, terminal output, and agent conversational memory.
- **Mitigation**: Implement custom `fmt::Debug` and `fmt::Display` for `TokenMetadata` masking `token` with `self.redacted_token()`.

### [High] Challenge 2: Short-Secret Exfiltration via Redacted Handle Suffix
- **Assumption challenged**: That the 8-character entropy suffix never exposes the full credential.
- **Attack scenario**: An attacker provisions a short secret (e.g., 8-character API key) and inspects model prompt logs containing `[REDACTED-TOKEN-12345678]`.
- **Blast radius**: The attacker recovers 100% of the raw credential bytes directly from the "sanitized" handle.
- **Mitigation**: Mask tokens `<= 8` chars with `...`.
