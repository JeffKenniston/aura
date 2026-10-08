use serde::{Deserialize, Serialize};

/// Short-lived credentials matching aura:tunnel WIT definition (TOOL-ET-001).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credentials {
    pub token: String,
    pub expires_at: u64,
}

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
        let suffix = if self.token.len() >= 8 {
            &self.token[self.token.len() - 8..]
        } else {
            "..."
        };
        format!("spiffe://aura.local/ephemeral/{}/[REDACTED-TOKEN-{}]", self.service, suffix)
    }
}
