use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use crate::credentials::{Credentials, TokenMetadata};

/// Default TTL for short-lived task credentials (2 minutes / 120 seconds)
pub const DEFAULT_CREDENTIAL_TTL_SECS: u64 = 120;

/// Service managing SPIRE dynamic credential minting, scope non-escalation,
/// and explicit token revocation (TOOL-ET-001, TOOL-ET-002, TOOL-ET-003).
#[derive(Clone, Debug)]
pub struct EphemeralTunnelService {
    parent_spiffe_id: String,
    allowed_services: HashSet<String>,
    default_ttl_secs: u64,
    tokens: Arc<Mutex<HashMap<String, TokenMetadata>>>,
}

impl Default for EphemeralTunnelService {
    fn default() -> Self {
        let parent_id = std::env::var("SPIFFE_ID")
            .unwrap_or_else(|_| "spiffe://aura.local/agent/default".to_string());
        Self::new(parent_id)
    }
}

impl EphemeralTunnelService {
    /// Creates a new `EphemeralTunnelService` bound to an attested parent SPIFFE ID.
    pub fn new(parent_spiffe_id: impl Into<String>) -> Self {
        let mut allowed = HashSet::new();
        allowed.insert("aws-sts".to_string());
        allowed.insert("vault".to_string());
        allowed.insert("spire".to_string());
        allowed.insert("github".to_string());
        allowed.insert("database".to_string());
        allowed.insert("cloud".to_string());
        allowed.insert("test-service".to_string());

        Self {
            parent_spiffe_id: parent_spiffe_id.into(),
            allowed_services: allowed,
            default_ttl_secs: DEFAULT_CREDENTIAL_TTL_SECS,
            tokens: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Configures allowed services for scope narrowing (TOOL-ET-002).
    pub fn with_allowed_services(mut self, services: Vec<String>) -> Self {
        self.allowed_services = services.into_iter().collect();
        self
    }

    /// Configures custom credential TTL in seconds.
    pub fn with_ttl_secs(mut self, ttl: u64) -> Self {
        self.default_ttl_secs = ttl;
        self
    }

    /// Returns the current Unix timestamp in seconds.
    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    /// Mints short-lived credentials valid only for the duration of the task (TOOL-ET-001).
    /// Enforces non-escalation invariant: requests outside allowed service scopes are rejected (TOOL-ET-002).
    pub fn mint_credentials(&self, service: &str) -> Result<Credentials, String> {
        let trimmed_service = service.trim();
        if trimmed_service.is_empty() {
            return Err("Service name cannot be empty".to_string());
        }

        // Scope non-escalation check (IAM-004, TOOL-ET-002)
        if !self.allowed_services.contains(trimmed_service) {
            return Err(format!(
                "Privilege escalation denied: service '{}' is not authorized under parent identity '{}' (allowed: {:?})",
                trimmed_service, self.parent_spiffe_id, self.allowed_services
            ));
        }

        let now = Self::current_timestamp();
        let expires_at = now + self.default_ttl_secs;

        // Generate dynamic cryptographically isolated token SVID
        let token_id = Uuid::new_v4().to_string();
        let token = format!(
            "spiffe://aura.local/ephemeral/{}/{}",
            trimmed_service, token_id
        );

        let metadata = TokenMetadata {
            token: token.clone(),
            service: trimmed_service.to_string(),
            issued_at: now,
            expires_at,
            revoked: false,
            scopes: vec![format!("scope:{}", trimmed_service)],
        };

        let mut lock = self
            .tokens
            .lock()
            .map_err(|e| format!("Lock acquisition failed: {}", e))?;
        lock.insert(token.clone(), metadata);

        Ok(Credentials { token, expires_at })
    }

    /// Explicitly revokes active credentials upon task completion, cancellation, or failure (TOOL-ET-003).
    pub fn revoke_credentials(&self, token: &str) -> Result<(), String> {
        let mut lock = self
            .tokens
            .lock()
            .map_err(|e| format!("Lock acquisition failed: {}", e))?;

        match lock.get_mut(token) {
            Some(meta) => {
                if meta.revoked {
                    return Err(format!("Token '{}' has already been revoked", token));
                }
                meta.revoked = true;
                Ok(())
            }
            None => Err(format!(
                "Token '{}' not found in active credentials registry",
                token
            )),
        }
    }

    /// Validates an ephemeral token against revocation and expiration status.
    pub fn verify_token(&self, token: &str) -> Result<TokenMetadata, String> {
        let lock = self
            .tokens
            .lock()
            .map_err(|e| format!("Lock acquisition failed: {}", e))?;

        let meta = lock
            .get(token)
            .ok_or_else(|| format!("Token '{}' not found", token))?;

        if meta.revoked {
            return Err(format!("Token '{}' is revoked", token));
        }

        let now = Self::current_timestamp();
        if now >= meta.expires_at {
            return Err(format!(
                "Token '{}' expired at {} (current time: {})",
                token, meta.expires_at, now
            ));
        }

        Ok(meta.clone())
    }

    /// Sanitizes token for model context, ensuring raw secrets never leak into LLM prompts (TOOL-ET-002).
    pub fn sanitize_for_prompt(&self, token: &str) -> String {
        let lock = match self.tokens.lock() {
            Ok(l) => l,
            Err(_) => return "[REDACTED-TOKEN]".to_string(),
        };

        lock.get(token)
            .map(|m| m.redacted_token())
            .unwrap_or_else(|| "[REDACTED-TOKEN]".to_string())
    }
}
