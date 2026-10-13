# Strategy Report: TokenMetadata Debug & Display Formatting Remediation

**Agent:** `explorer_m1_it2_1`  
**Roles:** explorer, investigator  
**Target:** `tools/ephemeral-tunnel/src/credentials.rs`  
**Date:** 2026-10-11T18:18:00Z  
**Working Directory:** `/home/jeff/aura/.agents/teamwork/explorer_m1_it2_1`  

---

## 1. Executive Summary & Defect Root Cause

### 1.1 Defect Description
In Gate 1 Iteration 1 of Phase M5 (Hardening), independent code review (`reviewer_m1_1`) and empirical adversarial testing (`challenger_m1_1`) identified a critical zero-trust isolation defect in `tools/ephemeral-tunnel/src/credentials.rs`:
- `TokenMetadata` derived the compiler-default `Debug` macro via `#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]`.
- `TokenMetadata` did NOT implement `std::fmt::Display`.
- Consequently, invoking standard debug formatting `format!("{:?}", metadata)` or pretty debug formatting `format!("{:#?}", metadata)` emitted the raw, unredacted SPIFFE token string in plaintext (e.g. `TokenMetadata { token: "spiffe://aura.local/ephemeral/github/RAW_SECRET_...", ... }`).

### 1.2 Security & Contractual Impact
- **Breach of Invariant IAM-004 & TOOL-ET-002**: Ephemeral secrets minted by `EphemeralTunnelService` must never appear in diagnostic logs, tracing spans, or LLM context formatters.
- **PROJECT Feature 1 Contract**: Requires custom `Display` and `Debug` implementations producing opaque `[REDACTED-TOKEN-...]` handles.
- **Worker Claim Refuted**: `worker_m1_2` claimed to have implemented custom `Display` and `Debug` for `TokenMetadata`. Empirical inspection proved `TokenMetadata` still had default derived `Debug` and lacked `Display`.

---

## 2. Technical Analysis & Solution Architecture

### 2.1 Struct Layout & Field Classification
`TokenMetadata` holds six fields (`tools/ephemeral-tunnel/src/credentials.rs:57-64`):
```rust
pub struct TokenMetadata {
    pub token: String,        // SENSITIVE: raw SPIFFE secret token
    pub service: String,      // NON-SENSITIVE: target service identifier (e.g. "github", "aws-sts")
    pub issued_at: u64,       // NON-SENSITIVE: creation Unix timestamp
    pub expires_at: u64,      // NON-SENSITIVE: expiration Unix timestamp
    pub revoked: bool,        // NON-SENSITIVE: lifecycle status flag
    pub scopes: Vec<String>,  // NON-SENSITIVE: authorized capability scopes
}
```

### 2.2 Symmetrical Alignment with `Credentials`
`Credentials` in lines 40–53 already demonstrates the desired pattern:
```rust
impl fmt::Display for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.redacted_handle())
    }
}

impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credentials")
            .field("token", &self.redacted_handle())
            .field("expires_at", &self.expires_at)
            .finish()
    }
}
```

Applying this same architectural pattern to `TokenMetadata`:
1. `TokenMetadata::redacted_token(&self)` returns `spiffe://aura.local/ephemeral/{service}/[REDACTED-TOKEN-{suffix}]`.
2. `Display` delegates directly to `write!(f, "{}", self.redacted_token())`.
3. `Debug` uses `f.debug_struct("TokenMetadata")`, replacing the sensitive `token` field with `&self.redacted_token()` while faithfully formatting all non-sensitive fields (`service`, `issued_at`, `expires_at`, `revoked`, `scopes`).
4. Using `f.debug_struct` preserves standard Rust formatting semantics, correctly supporting both compact `{:?}` and multiline indented `{:#?}` without manual string manipulation.

### 2.3 Retention of Non-Sensitive Fields
The dispatch instructions noted: *"preserving non-sensitive fields (`service`, `expires_at`, `scopes`, `revoked`)"*.  
`issued_at` is an essential non-sensitive timestamp (`pub issued_at: u64`) that was omitted parenthetically in the dispatch prompt. Retaining `issued_at` in the `debug_struct` builder:
- Maintains full field parity with the struct declaration and previous derived `Debug` output.
- Emits no sensitive data (it is a standard Unix epoch seconds integer).
- Prevents breaking external diagnostic tools or logs that track token issuance timestamps.

---

## 3. Exact Drop-In Code Fix

### 3.1 Proposed Code Modification (`tools/ephemeral-tunnel/src/credentials.rs`)

```rust
// ==========================================
// BEFORE (Lines 55-84):
// ==========================================
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

impl TokenMetadata {
    /// Checks if the token is currently active and within valid TTL.
    pub fn is_valid(&self, current_time: u64) -> bool {
        !self.revoked && current_time < self.expires_at
    }

    /// Generates a sanitized/redacted token representation for model logs,
    /// ensuring sensitive raw tokens stay completely out of LLM context (TOOL-ET-002).
    pub fn redacted_token(&self) -> String {
        let chars: Vec<char> = self.token.chars().collect();
        let suffix: String = if chars.len() >= 8 {
            chars[chars.len() - 8..].iter().collect()
        } else {
            "...".to_string()
        };
        format!("spiffe://aura.local/ephemeral/{}/[REDACTED-TOKEN-{}]", self.service, suffix)
    }
}

// ==========================================
// AFTER:
// ==========================================
/// Metadata tracked for an active ephemeral token (TOOL-ET-001, TOOL-ET-002, TOOL-ET-003).
///
/// NOTE: Serialize/Deserialize preserves raw credentials for out-of-band transport only.
/// For logging, tracing, or model context formatting, use `Display` or `Debug`.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenMetadata {
    pub token: String,
    pub service: String,
    pub issued_at: u64,
    pub expires_at: u64,
    pub revoked: bool,
    pub scopes: Vec<String>,
}

impl TokenMetadata {
    /// Checks if the token is currently active and within valid TTL.
    pub fn is_valid(&self, current_time: u64) -> bool {
        !self.revoked && current_time < self.expires_at
    }

    /// Generates a sanitized/redacted token representation for model logs,
    /// ensuring sensitive raw tokens stay completely out of LLM context (TOOL-ET-002).
    pub fn redacted_token(&self) -> String {
        let chars: Vec<char> = self.token.chars().collect();
        let suffix: String = if chars.len() >= 8 {
            chars[chars.len() - 8..].iter().collect()
        } else {
            "...".to_string()
        };
        format!("spiffe://aura.local/ephemeral/{}/[REDACTED-TOKEN-{}]", self.service, suffix)
    }
}

impl fmt::Display for TokenMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.redacted_token())
    }
}

impl fmt::Debug for TokenMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TokenMetadata")
            .field("token", &self.redacted_token())
            .field("service", &self.service)
            .field("issued_at", &self.issued_at)
            .field("expires_at", &self.expires_at)
            .field("revoked", &self.revoked)
            .field("scopes", &self.scopes)
            .finish()
    }
}
```

---

## 4. Unified Diff Patch

```diff
--- a/tools/ephemeral-tunnel/src/credentials.rs
+++ b/tools/ephemeral-tunnel/src/credentials.rs
@@ -53,7 +53,10 @@ impl fmt::Debug for Credentials {
 }
 
 /// Metadata tracked for an active ephemeral token (TOOL-ET-001, TOOL-ET-002, TOOL-ET-003).
-#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
+///
+/// NOTE: Serialize/Deserialize preserves raw credentials for out-of-band transport only.
+/// For logging, tracing, or model context formatting, use `Display` or `Debug`.
+#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
 pub struct TokenMetadata {
     pub token: String,
     pub service: String,
@@ -83,3 +86,21 @@ impl TokenMetadata {
     }
 }
 
+impl fmt::Display for TokenMetadata {
+    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
+        write!(f, "{}", self.redacted_token())
+    }
+}
+
+impl fmt::Debug for TokenMetadata {
+    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
+        f.debug_struct("TokenMetadata")
+            .field("token", &self.redacted_token())
+            .field("service", &self.service)
+            .field("issued_at", &self.issued_at)
+            .field("expires_at", &self.expires_at)
+            .field("revoked", &self.revoked)
+            .field("scopes", &self.scopes)
+            .finish()
+    }
+}
```

---

## 5. Interaction with Peer Explorers

1. **`explorer_m1_it2_2` (Boundary Logic)**:
   - `explorer_m1_it2_2` modifies `redacted_token()` in `TokenMetadata` (e.g., clamping tokens with `chars.len() <= 8` to `...`).
   - Because our `fmt::Display` and `fmt::Debug` implementations invoke `self.redacted_token()`, any boundary logic improvements applied to `redacted_token()` are automatically inherited with zero coupling or conflict.

2. **`explorer_m1_it2_3` (Test Suite Formulations)**:
   - `explorer_m1_it2_3` is constructing companion tests in `tests/tunnel_test.rs`:
     - `test_token_metadata_display_and_debug_redaction`
     - asserting `Display`, `Debug`, and `{:#?}` never leak raw secret strings while preserving metadata fields.
   - Our proposed implementation satisfies every assertion designed by `explorer_m1_it2_3`.

---

## 6. Serde Serialization Evaluation

Both `reviewer_m1_1` (Finding 3) and `challenger_m1_1` (Defect 2) noted that `serde_json::to_string(&metadata)` outputs the raw token.
- **Architectural Requirement**: `TokenMetadata` must retain its raw secret in `serde` serialization to support internal encrypted persistence (AES-256-GCM storage in Feature 15/16) and out-of-band sandbox injection (Feature 5). If Serde masked the token, deserialization would irrecoverably corrupt the stored credentials.
- **Safety Policy**: Document explicitly on `TokenMetadata` and `Credentials` that `Serialize`/`Deserialize` is reserved strictly for out-of-band cryptographic and runtime operations, while model prompt and diagnostic formatting MUST strictly use `Display` (`{}`) or `Debug` (`{:?}`).
