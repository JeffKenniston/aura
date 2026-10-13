# Empirical Adversarial Challenge Report: Milestone 1 Credential Formatting & WASM Persistence

**Agent:** `challenger_m1_1`  
**Working Directory:** `/home/jeff/aura/.agents/teamwork/challenger_m1_1`  
**Date:** 2026-10-11T18:12:00Z  
**Type:** Hard Handoff  
**Verdict:** **REJECT**

---

## 1. Observation

Direct empirical observations from source inspection and execution of the adversarial stress harness against `tools/ephemeral-tunnel`:

### 1.1 Source Code Observations

1. **`tools/ephemeral-tunnel/src/credentials.rs:56-64`**:
   `TokenMetadata` is defined as:
   ```rust
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
   - `TokenMetadata` derives `Debug` directly using the default compiler derive.
   - `TokenMetadata` does NOT implement `std::fmt::Display`.
   - `TokenMetadata` derives `serde::Serialize` and `serde::Deserialize` directly.

2. **`tools/ephemeral-tunnel/src/credentials.rs:14-20`**:
   Suffix extraction in `Credentials::redacted_handle()`:
   ```rust
   let chars: Vec<char> = self.token.chars().collect();
   let suffix: String = if chars.len() >= 8 {
       chars[chars.len() - 8..].iter().collect()
   } else {
       "...".to_string()
   };
   ```
   - When a token has length exactly 8 characters (e.g. `"12345678"`), `chars.len() >= 8` evaluates to true, taking `chars[0..8]`. The suffix is identical to the entire raw secret.

3. **`worker_m1_2/handoff.md:88`**:
   Worker `worker_m1_2` explicitly stated in their Logic Chain:
   > "Task 1 (Ephemeral Tunnel): `Credentials` and `TokenMetadata` require that raw secrets never appear in logs or context formatters (PROJECT Feature 1, IAM-004). Implementing custom `Display` and `Debug` in `credentials.rs` guarantees that any formatting yields `[REDACTED-TOKEN-...]`."

### 1.2 Test Harness Execution Results

1. **`TokenMetadata` Debug and Pretty-Debug Leakage**:
   Running `cargo test --manifest-path tools/ephemeral-tunnel/Cargo.toml --test adversarial_stress_test` against a `TokenMetadata` instance with raw secret `"SECRET_VALUE_XYZ_98765432"` yielded:
   ```
   thread 'assert_token_metadata_debug_does_not_leak' panicked:
   TokenMetadata Debug leaked raw token: TokenMetadata { token: "spiffe://aura.local/ephemeral/github/SECRET_VALUE_XYZ_98765432", service: "github", issued_at: 1000, expires_at: 2000, revoked: false, scopes: ["scope:github"] }

   thread 'assert_token_metadata_pretty_debug_does_not_leak' panicked:
   TokenMetadata Pretty Debug leaked raw token: TokenMetadata {
       token: "spiffe://aura.local/ephemeral/github/SECRET_VALUE_XYZ_98765432",
       service: "github",
       issued_at: 1000,
       expires_at: 2000,
       revoked: false,
       scopes: [
           "scope:github",
       ],
   }
   ```
   **Observation**: Default derived `Debug` emits the raw token in plaintext. Any logging or formatting of `TokenMetadata` via `{:?}` or `{:#?}` leaks credentials out to logs/model context.

2. **Serde Serialization Leakage**:
   Executing Serde serialization via `serde_json::to_string`:
   ```
   Credentials Serde JSON: {"token":"spiffe://aura.local/ephemeral/github/SECRET_VALUE_XYZ_98765432","expires_at":2000}
   TokenMetadata Serde JSON: {"token":"spiffe://aura.local/ephemeral/github/SECRET_VALUE_XYZ_98765432","service":"github","issued_at":1000,"expires_at":2000,"revoked":false,"scopes":["scope:github"]}
   ```
   **Observation**: Both `Credentials` and `TokenMetadata` serialize raw secret tokens into JSON in plaintext.

3. **8-Character Boundary Leakage**:
   When testing an 8-character token `"12345678"`:
   ```
   [exact_8] Display output: [REDACTED-TOKEN-12345678]
   ```
   **Observation**: 100% of the raw secret bytes were exposed in the sanitized handle because the suffix calculation takes `chars[len - 8..]` whenever `len >= 8`.

4. **Multi-Threaded Concurrency & Double-Revocation Race Test**:
   - 16 threads executing 1,600 interleaved operations across authorized and unauthorized services:
     ```
     Concurrent stress results: minted=1280, mint_denied=320, revoked=1280, revoke_failed=0
     ```
   - 10 threads racing to revoke the same token concurrently: exactly 1 thread succeeded, 9 threads received `Err("Token '...' has already been revoked")`.
   - 0 deadlocks, 0 lock poisoning panics, 0 dropped revocations.

5. **Clean Working Tree Test**:
   After removing temporary test file `tools/ephemeral-tunnel/tests/adversarial_stress_test.rs`:
   `cargo test --manifest-path tools/ephemeral-tunnel/Cargo.toml` executed:
   ```
   test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
   Doc-tests ephemeral_tunnel: 1 passed; 0 failed.
   ```

---

## 2. Logic Chain

1. **Disproved Worker Claim on `TokenMetadata`** (from Observation 1.1.1, 1.1.3, 1.2.1):
   - Worker `worker_m1_2` claimed that `TokenMetadata` implemented custom `Display` and `Debug` formatting to ensure raw secrets never leak in logs or formatters (IAM-004, TOOL-ET-002).
   - In `tools/ephemeral-tunnel/src/credentials.rs:56`, `TokenMetadata` derives `Debug` without masking and does not implement `Display`.
   - Calling `format!("{:?}", meta)` outputs the raw secret string in plaintext. Thus, worker `worker_m1_2`'s claim is refuted empirically, and `TokenMetadata` is a live secret leak vector.

2. **Serde Serialization Secret Leak** (from Observation 1.1.1, 1.2.2):
   - The task specification requires asserting that `Credentials` and `TokenMetadata` formatting under `Display`, `Debug`, `{:#?}`, and Serde serialization never leak raw tokens.
   - Both structs derive `serde::Serialize` on their public fields, including `pub token: String`.
   - Serializing either struct via `serde_json::to_string` produces plaintext JSON containing the unmasked token.

3. **Entropy Suffix Boundary Leak for Short Tokens** (from Observation 1.1.2, 1.2.3):
   - For any token whose length is exactly 8 characters, the suffix constitutes 100% of the token.
   - Revealing 8 characters of an 8-character token reveals the entire secret in the "redacted" string `[REDACTED-TOKEN-12345678]`.

4. **Service Concurrency & WASM Persistence** (from Observation 1.2.4, 1.2.5):
   - The thread-safe `OnceLock` singleton `global_service()` correctly serializes access to `tokens: Arc<Mutex<HashMap<String, TokenMetadata>>>`.
   - High contention (16 threads, 1600 operations) demonstrates thread safety, non-escalation enforcement, and atomic double-revocation handling without lock contention failures.

---

## 3. Caveats

- In real deployments, ephemeral tokens generated by `EphemeralTunnelService` have the format `spiffe://aura.local/ephemeral/{service}/{uuid}` (typically ~70 characters long), which mitigates the 8-character token edge case in practice. However, `Credentials` and `TokenMetadata` accept arbitrary user-supplied tokens where short or 8-character values can occur.
- Serde serialization may theoretically be required for internal state persistence (e.g. encrypted storage), but exposing `Serialize` without either custom serialization (to redact or encrypt) or documented isolation leaves external JSON-RPC/MCP handlers vulnerable to leaking raw tokens over JSON serialization.
- No modifications were made to implementation files per the review-only constraint.

---

## 4. Conclusion

**Verdict: REJECT**

While the concurrency locking and WASM singleton persistence of `EphemeralTunnelService` passed all adversarial stress tests with zero regressions, Milestone 1 cannot be approved in its current state due to critical secret leakage defects:

1. **DEFECT 1 (High)**: `TokenMetadata` lacks custom `Debug` and `Display` implementations, deriving standard `Debug`. Formatter calls `format!("{:?}", meta)` and `format!("{:#?}", meta)` emit plaintext raw secret tokens in violation of IAM-004, TOOL-ET-002, and contradictory to worker claims.
2. **DEFECT 2 (Medium)**: Serde serialization on `Credentials` and `TokenMetadata` serializes the raw secret token in plaintext.
3. **DEFECT 3 (Low/Medium)**: Tokens with length == 8 characters leak 100% of their secret bytes in `[REDACTED-TOKEN-{suffix}]`. Tokens should require length > 8 (e.g., length >= 16) before suffix exposure, or mask tokens under 16 characters with `...`.

### Actionable Remediation Required for Approval
1. In `tools/ephemeral-tunnel/src/credentials.rs`:
   - Implement custom `fmt::Debug` and `fmt::Display` for `TokenMetadata` that masks `token` with `self.redacted_token()`.
   - Update `redacted_handle()` / `redacted_token()` so that if `chars.len() <= 8` (or `< 12`), the suffix is `...` rather than exposing the entire token.
   - Either implement custom `Serialize` for `Credentials` and `TokenMetadata` returning `redacted_handle()` or document/restrict serialization attributes to avoid accidental JSON leakage.

---

## 5. Verification Method

To independently reproduce the empirical findings:

1. Inspect `TokenMetadata` derive attributes:
   ```bash
   sed -n '55,66p' /home/jeff/aura/tools/ephemeral-tunnel/src/credentials.rs
   ```
2. Verify that `TokenMetadata` derives `Debug` without custom masking, whereas `Credentials` implements custom `Debug` at line 46.
3. Verify existing baseline test suite remains passing:
   ```bash
   cargo test --manifest-path tools/ephemeral-tunnel/Cargo.toml
   ```
   Expected: 10 unit/integration tests pass, 1 doc test passes.
