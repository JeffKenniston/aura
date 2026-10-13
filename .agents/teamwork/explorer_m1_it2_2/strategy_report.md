# Strategy Report: Token Redaction Suffix Boundary & Serde Serialization Remediation

**Agent:** `explorer_m1_it2_2`  
**Roles:** explorer, investigator, security architect  
**Target File:** `tools/ephemeral-tunnel/src/credentials.rs`  
**Date:** 2026-10-11T18:22:00Z  
**Working Directory:** `/home/jeff/aura/.agents/teamwork/explorer_m1_it2_2`  

---

## 1. Executive Summary & Defect Root Cause

### 1.1 The Vulnerability
In Gate 1 Iteration 1 of Phase M5 (Hardening), independent code review (`reviewer_m1_1`) and empirical adversarial stress testing (`challenger_m1_1`) identified a critical secret exposure defect in the token redaction logic within `tools/ephemeral-tunnel/src/credentials.rs`:
- Lines 16–20 (`Credentials::redacted_handle`):
  ```rust
  let chars: Vec<char> = self.token.chars().collect();
  let suffix: String = if chars.len() >= 8 {
      chars[chars.len() - 8..].iter().collect()
  } else {
      "...".to_string()
  };
  ```
- Lines 76–80 (`TokenMetadata::redacted_token`):
  ```rust
  let chars: Vec<char> = self.token.chars().collect();
  let suffix: String = if chars.len() >= 8 {
      chars[chars.len() - 8..].iter().collect()
  } else {
      "...".to_string()
  };
  ```

### 1.2 Mathematical & Security Breakdown
When an arbitrary 8-character token (such as `"12345678"`, `"password"`, or `"api_key1"`) is formatted:
1. `chars.len()` evaluates to `8`.
2. The condition `chars.len() >= 8` evaluates to `true`.
3. The slice `chars[chars.len() - 8..]` evaluates to `chars[0..8]`, capturing **all 8 characters** of the secret.
4. The sanitized output emitted to logs, telemetry, and model context is `[REDACTED-TOKEN-12345678]`.
5. **Zero redaction occurs. 100% of raw secret bytes are disclosed in the sanitized handle.**

### 1.3 Secondary Vulnerability: SPIFFE-Wrapped Short Secrets
Investigation revealed a subtle secondary bypass:
- If a short secret (e.g. `"12345678"`) is encapsulated in a SPIFFE URI:
  `spiffe://aura.local/ephemeral/vault/12345678`
- The entire URI is 44 characters long.
- Naive string length checks on `self.token` evaluate `44 >= 8` (and `44 >= 16`) as `true`.
- The trailing 8 characters extracted are `"12345678"`, again exposing 100% of the secret payload:
  `spiffe://aura.local/ephemeral/vault/[REDACTED-TOKEN-12345678]`.
- **Solution Requirement**: Length clamping must evaluate the **secret payload component**, stripping the SPIFFE scheme prefix (`spiffe://aura.local/ephemeral/{service}/`) before computing the suffix.

### 1.4 Test Suite Regression Flaw
In `tools/ephemeral-tunnel/tests/tunnel_test.rs:171-177`:
```rust
// 3. Exact 8 characters
let eight_creds = Credentials {
    token: "12345678".to_string(),
    expires_at: 100,
};
let eight_display = format!("{}", eight_creds);
assert_eq!(eight_display, "[REDACTED-TOKEN-12345678]");
```
The test suite explicitly encoded the defect as expected behavior! Updating the implementation to clamp 8-character tokens requires reconciling this assertion to expect `"[REDACTED-TOKEN-...]"`.

---

## 2. Suffix Boundary Remediation: Threshold Analysis

### 2.1 Comparison of Clamping Thresholds

| Property | Threshold A: `chars.len() > 8` | Threshold B: `chars.len() >= 16` |
|---|---|---|
| **Exact 8-char token (`"12345678"`)** | Clamped to `"..."` (`[REDACTED-TOKEN-...]`) | Clamped to `"..."` (`[REDACTED-TOKEN-...]`) |
| **Short token (`"secret"`, len 6)** | Clamped to `"..."` (`[REDACTED-TOKEN-...]`) | Clamped to `"..."` (`[REDACTED-TOKEN-...]`) |
| **Empty token (`""`, len 0)** | Clamped to `"..."` (`[REDACTED-TOKEN-...]`) | Clamped to `"..."` (`[REDACTED-TOKEN-...]`) |
| **9..15 char tokens** | Displays trailing 8 chars (1..7 chars masked) | Clamped to `"..."` (`[REDACTED-TOKEN-...]`) |
| **Brute-Force Attack Resistance** | For len 9, only 1 char hidden (~62 guesses) | Guarantees >= 8 chars hidden (>= 2.18 × 10^14 guesses) |
| **Multi-byte UTF-8 Test (`token-🔑🔒🛡️`, len 10)** | Exercises unicode slice `chars[2..]` (8 chars) | Clamped to `"..."` unless test token extended |
| **Production Aura SVIDs (~75 chars)** | Preserves 8-char correlation suffix (`89abcdef`) | Preserves 8-char correlation suffix (`89abcdef`) |

### 2.2 Recommendation
- **Primary Recommendation**: Threshold `chars.len() > 8` directly resolves the defect reported by `reviewer_m1_1` and `challenger_m1_1` with minimal disruption, while immediately exercising multi-byte unicode slicing in existing edge-case tests.
- **Defense-in-Depth Alternative**: Threshold `chars.len() >= 16` is cryptographically stricter, ensuring that at least 8 characters (>= 50% entropy) remain masked on any token that displays a suffix.
- Both thresholds are fully formulated in Section 4.

---

## 3. Serde Serialization Comprehensive Evaluation

### 3.1 Architectural Context & Usage Survey
A complete grep survey across the workspace confirms:
1. `Credentials` and `TokenMetadata` are only used in `tools/ephemeral-tunnel`.
2. Out-of-band sandbox injection in `aura-core` (`BashSessionManager::inject_credential`) passes `raw_secret: &str` directly via in-memory function calls.
3. WASM component export in `tools/ephemeral-tunnel/src/lib.rs` transmits credentials using native WIT record types (`aura:tunnel` interface), not JSON.
4. Neither `Credentials` nor `TokenMetadata` is deserialized from JSON anywhere in the existing codebase.

### 3.2 Evaluation of Serde Models

#### Model A: Documented Internal Transport (Derived Serde + Explicit Invariant Documentation)
- **Mechanism**: Retain `#[derive(Serialize, Deserialize)]` on `Credentials` and `TokenMetadata`. Add prominent security doc comments stating that Serde is reserved strictly for internal/out-of-band state serialization, while public, logging, and model-facing representations MUST use `Display` (`{}`) or `Debug` (`{:?}`).
- **Pros**:
  - Preserves round-trip serialization fidelity (`from_str(to_string(x)) == x`).
  - Supports future encrypted persistence at rest (e.g. storing raw tokens inside AES-256-GCM encrypted database envelopes).
  - Aligns with `explorer_m1_it2_1`'s design.
- **Cons**:
  - If a developer carelessly passes `&creds` to an MCP JSON-RPC response or JSON logger, raw secrets are emitted in JSON format.

#### Model B: Zero-Trust Hermetic Masking (Custom Serde Serializer Emitting Redacted Handles)
- **Mechanism**: Implement custom `serde::Serialize` for `Credentials` and `TokenMetadata` that serializes the `"token"` field as `self.redacted_handle()` / `self.redacted_token()`. Keep `#[derive(Deserialize)]` or manual deserializer.
- **Pros**:
  - 100% fail-safe: Even if an engineer serializes `Credentials` directly to a model-facing JSON response, `serde_json::to_string` NEVER discloses raw secret bytes.
  - Satisfies adversarial test requirements asserting that `serde_json::to_string` contains `[REDACTED-TOKEN-...]` and zero secret bytes.
- **Cons**:
  - Breaks round-trip deserialization: deserializing the JSON produces a struct containing the redacted handle rather than the original raw credential.

#### Model C: Segregated Types / Public View Pattern
- **Mechanism**: `Credentials` and `TokenMetadata` retain raw fields with Serde derive, but provide dedicated safe view types:
  ```rust
  #[derive(Serialize)]
  pub struct RedactedCredentials {
      pub token: String,
      pub expires_at: u64,
  }
  impl From<&Credentials> for RedactedCredentials {
      fn from(c: &Credentials) -> Self {
          Self { token: c.redacted_handle(), expires_at: c.expires_at }
      }
  }
  ```
- **Pros**: Clear separation of internal transport vs external presentation.

### 3.3 Serde Synthesis & Recommendation
For Gate 1 Iteration 2, the cleanest path is:
1. Provide the **Documented Internal Transport Model (Model A)** as the primary architecture (fully unified with `explorer_m1_it2_1`), where `Display` and `Debug` are the zero-trust redaction boundaries.
2. Provide the **Custom Hermetic Serializer (Model B)** as a drop-in option if the reviewer or challenger requires that `serde_json::to_string` strictly mask secrets.

---

## 4. Exact Drop-In Code Formulations

### 4.1 Formulation 1 (Recommended): Unified Helper with SPIFFE Secret Extraction

This formulation introduces `extract_redacted_suffix(token: &str)` to guarantee that:
1. Tokens of length <= 8 characters are clamped to `"..."`.
2. SPIFFE-wrapped short tokens (e.g. `spiffe://aura.local/ephemeral/vault/12345678`) have their secret component isolated before length checking, preventing prefix length leakage.
3. Multi-byte UTF-8 characters are safely collected into `Vec<char>` before slicing.

```rust
/// Helper to extract a safe entropy suffix for redacted token handles (TOOL-ET-002).
///
/// Security invariants:
/// - Tokens / secrets with character count <= 8 clamp to `"..."` to prevent secret exposure.
/// - Suffix is only emitted when character count > 8.
/// - If the token is a SPIFFE URI (`spiffe://aura.local/ephemeral/{service}/{secret}`),
///   evaluates the secret component rather than the overall URI length.
pub(crate) fn extract_redacted_suffix(token: &str) -> String {
    let secret = if let Some(rest) = token.strip_prefix("spiffe://aura.local/ephemeral/") {
        if let Some((_, s)) = rest.split_once('/') {
            s
        } else {
            token
        }
    } else {
        token
    };

    let chars: Vec<char> = secret.chars().collect();
    if chars.len() > 8 {
        chars[chars.len() - 8..].iter().collect()
    } else {
        "...".to_string()
    }
}
```

#### Updated `Credentials::redacted_handle`
```rust
impl Credentials {
    /// Generates an opaque, sanitized representation of the credentials (TOOL-ET-002).
    /// Raw secrets are never exposed to logs or model context.
    pub fn redacted_handle(&self) -> String {
        let suffix = extract_redacted_suffix(&self.token);

        if let Some(rest) = self.token.strip_prefix("spiffe://aura.local/ephemeral/") {
            if let Some((service, _)) = rest.split_once('/') {
                return format!(
                    "spiffe://aura.local/ephemeral/{}/[REDACTED-TOKEN-{}]",
                    service, suffix
                );
            }
        }

        format!("[REDACTED-TOKEN-{}]", suffix)
    }

    /// Alias for `redacted_handle` for backwards compatibility.
    pub fn redacted_token(&self) -> String {
        self.redacted_handle()
    }
}
```

#### Updated `TokenMetadata::redacted_token`
```rust
impl TokenMetadata {
    /// Checks if the token is currently active and within valid TTL.
    pub fn is_valid(&self, current_time: u64) -> bool {
        !self.revoked && current_time < self.expires_at
    }

    /// Generates a sanitized/redacted token representation for model logs,
    /// ensuring sensitive raw tokens stay completely out of LLM context (TOOL-ET-002).
    pub fn redacted_token(&self) -> String {
        let suffix = extract_redacted_suffix(&self.token);
        format!(
            "spiffe://aura.local/ephemeral/{}/[REDACTED-TOKEN-{}]",
            self.service, suffix
        )
    }
}
```

---

### 4.2 Formulation 2: Minimal In-Line Diff (Localized Change)

If a minimal diff directly inside existing method bodies is preferred:

#### In `Credentials::redacted_handle`:
```rust
        let chars: Vec<char> = self.token.chars().collect();
        let suffix: String = if chars.len() > 8 {
            chars[chars.len() - 8..].iter().collect()
        } else {
            "...".to_string()
        };
```

#### In `TokenMetadata::redacted_token`:
```rust
        let chars: Vec<char> = self.token.chars().collect();
        let suffix: String = if chars.len() > 8 {
            chars[chars.len() - 8..].iter().collect()
        } else {
            "...".to_string()
        };
```

---

### 4.3 Formulation for Stricter 16-Character Boundary (`chars.len() >= 16`)

If the team opts for the 16-character boundary:
```rust
    let chars: Vec<char> = secret.chars().collect();
    if chars.len() >= 16 {
        chars[chars.len() - 8..].iter().collect()
    } else {
        "...".to_string()
    }
```

---

### 4.4 Drop-In Custom Serde Serializers (Model B)

If custom zero-trust JSON serialization is required:

```rust
use serde::ser::SerializeStruct;

impl Serialize for Credentials {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("Credentials", 2)?;
        state.serialize_field("token", &self.redacted_handle())?;
        state.serialize_field("expires_at", &self.expires_at)?;
        state.end()
    }
}

impl Serialize for TokenMetadata {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("TokenMetadata", 6)?;
        state.serialize_field("token", &self.redacted_token())?;
        state.serialize_field("service", &self.service)?;
        state.serialize_field("issued_at", &self.issued_at)?;
        state.serialize_field("expires_at", &self.expires_at)?;
        state.serialize_field("revoked", &self.revoked)?;
        state.serialize_field("scopes", &self.scopes)?;
        state.end()
    }
}
```

---

## 5. Complete Harmonized `credentials.rs` Source File

Combining `explorer_m1_it2_1`'s `TokenMetadata` Display/Debug fixes with `explorer_m1_it2_2`'s suffix boundary and Serde documentation produces the definitive drop-in replacement:

```rust
use std::fmt;
use serde::{Deserialize, Serialize};

/// Extracts a safe entropy suffix for redacted token handles (TOOL-ET-002).
///
/// Security invariants:
/// - Tokens / secrets with character count <= 8 clamp to `"..."` to prevent raw secret exposure.
/// - Suffix is only emitted when character count > 8.
/// - If the token is a SPIFFE URI (`spiffe://aura.local/ephemeral/{service}/{secret}`),
///   evaluates the secret component rather than the overall URI length.
pub(crate) fn extract_redacted_suffix(token: &str) -> String {
    let secret = if let Some(rest) = token.strip_prefix("spiffe://aura.local/ephemeral/") {
        if let Some((_, s)) = rest.split_once('/') {
            s
        } else {
            token
        }
    } else {
        token
    };

    let chars: Vec<char> = secret.chars().collect();
    if chars.len() > 8 {
        chars[chars.len() - 8..].iter().collect()
    } else {
        "...".to_string()
    }
}

/// Short-lived credentials matching aura:tunnel WIT definition (TOOL-ET-001).
///
/// NOTE: Serialize/Deserialize preserves raw credentials for out-of-band transport only.
/// For logging, tracing, or model context formatting, use `Display` or `Debug`.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credentials {
    pub token: String,
    pub expires_at: u64,
}

impl Credentials {
    /// Generates an opaque, sanitized representation of the credentials (TOOL-ET-002).
    /// Raw secrets are never exposed to logs or model context.
    pub fn redacted_handle(&self) -> String {
        let suffix = extract_redacted_suffix(&self.token);

        if let Some(rest) = self.token.strip_prefix("spiffe://aura.local/ephemeral/") {
            if let Some((service, _)) = rest.split_once('/') {
                return format!(
                    "spiffe://aura.local/ephemeral/{}/[REDACTED-TOKEN-{}]",
                    service, suffix
                );
            }
        }

        format!("[REDACTED-TOKEN-{}]", suffix)
    }

    /// Alias for `redacted_handle` for backwards compatibility.
    pub fn redacted_token(&self) -> String {
        self.redacted_handle()
    }
}

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
        let suffix = extract_redacted_suffix(&self.token);
        format!(
            "spiffe://aura.local/ephemeral/{}/[REDACTED-TOKEN-{}]",
            self.service, suffix
        )
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

## 6. Test Reconciliation Guidance for `explorer_m1_it2_3` & Worker

### 6.1 Test Assertion Update in `tests/tunnel_test.rs`
Update lines 171–177 from:
```rust
    // 3. Exact 8 characters
    let eight_creds = Credentials {
        token: "12345678".to_string(),
        expires_at: 100,
    };
    let eight_display = format!("{}", eight_creds);
    assert_eq!(eight_display, "[REDACTED-TOKEN-12345678]");
```
to:
```rust
    // 3. Exact 8 characters - clamped to "..." to guarantee 0 secret bytes exposed
    let eight_creds = Credentials {
        token: "12345678".to_string(),
        expires_at: 100,
    };
    let eight_display = format!("{}", eight_creds);
    assert_eq!(eight_display, "[REDACTED-TOKEN-...]");
    assert!(!eight_display.contains("12345678"));
```

### 6.2 New Recommended Boundary Test Cases
Add to `tests/tunnel_test.rs`:
```rust
#[test]
fn test_short_token_boundary_does_not_reveal_full_secret() {
    // 8-character bare token
    let creds_8 = Credentials {
        token: "12345678".to_string(),
        expires_at: 100,
    };
    assert_eq!(format!("{}", creds_8), "[REDACTED-TOKEN-...]");
    assert!(!format!("{}", creds_8).contains("12345678"));
    assert!(!format!("{:?}", creds_8).contains("12345678"));

    // 8-character secret inside SPIFFE URI
    let spiffe_8 = Credentials {
        token: "spiffe://aura.local/ephemeral/vault/12345678".to_string(),
        expires_at: 100,
    };
    assert_eq!(
        format!("{}", spiffe_8),
        "spiffe://aura.local/ephemeral/vault/[REDACTED-TOKEN-...]"
    );
    assert!(!format!("{}", spiffe_8).contains("12345678"));

    // 8-character secret in TokenMetadata
    let meta_8 = TokenMetadata {
        token: "12345678".to_string(),
        service: "vault".to_string(),
        issued_at: 100,
        expires_at: 200,
        revoked: false,
        scopes: vec!["scope:vault".to_string()],
    };
    assert_eq!(
        format!("{}", meta_8),
        "spiffe://aura.local/ephemeral/vault/[REDACTED-TOKEN-...]"
    );
    assert!(!format!("{}", meta_8).contains("12345678"));
    assert!(!format!("{:?}", meta_8).contains("12345678"));
}
```

---

## 7. Verification Method

1. **Verify Unit & Integration Tests**:
   ```bash
   cargo test --manifest-path tools/ephemeral-tunnel/Cargo.toml
   ```
2. **Verify Full M5 E2E Opaque Suite**:
   ```bash
   cargo test -p aura-core --test m5_e2e_opaque_test
   ```
3. **Verify Lints & Compilation**:
   ```bash
   cargo clippy --manifest-path tools/ephemeral-tunnel/Cargo.toml
   ```
