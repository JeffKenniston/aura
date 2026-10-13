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
