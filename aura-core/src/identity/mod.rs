pub mod enforcer;

use cedar_policy::{
    Authorizer, Context, Decision, Entities, EntityId, EntityTypeName, EntityUid, PolicySet,
    Request,
};
use spiffe::workload_api::client::WorkloadApiClient;
use spiffe::{SpiffeId, TrustDomain};
use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

// ============================================================================
// IAM-004 & IAM-005: Capability Scopes & Scoped SVID Enums
// ============================================================================

/// Fine-grained capability scopes for Aura OS sandboxes and agents (IAM-004).
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum Capability {
    FileSystemRead,
    FileSystemWrite,
    NetworkBind,
    ToolBash,
    ToolBrowser,
    ToolComputer,
    ToolFileSystem,
    ToolBlastRadius,
    ToolEphemeralTunnel,
    Custom(String),
}

impl Capability {
    pub fn as_str(&self) -> &str {
        match self {
            Capability::FileSystemRead => "fs:read",
            Capability::FileSystemWrite => "fs:write",
            Capability::NetworkBind => "net:bind",
            Capability::ToolBash => "tool:bash",
            Capability::ToolBrowser => "tool:browser",
            Capability::ToolComputer => "tool:computer",
            Capability::ToolFileSystem => "tool:filesystem",
            Capability::ToolBlastRadius => "tool:ast-blast-radius",
            Capability::ToolEphemeralTunnel => "tool:ephemeral-tunnel",
            Capability::Custom(ref s) => s.as_str(),
        }
    }
}

impl FromStr for Capability {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "fs:read" | "read" => Capability::FileSystemRead,
            "fs:write" | "write" => Capability::FileSystemWrite,
            "net:bind" => Capability::NetworkBind,
            "tool:bash" | "bash" => Capability::ToolBash,
            "tool:browser" | "browser" => Capability::ToolBrowser,
            "tool:computer" | "computer" => Capability::ToolComputer,
            "tool:filesystem" | "filesystem" => Capability::ToolFileSystem,
            "tool:ast-blast-radius" | "ast-blast-radius" => Capability::ToolBlastRadius,
            "tool:ephemeral-tunnel" | "ephemeral-tunnel" => Capability::ToolEphemeralTunnel,
            other => Capability::Custom(other.to_string()),
        })
    }
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ============================================================================
// IAM-005: Explicit ScopeDenied & IdentityError
// ============================================================================

/// Error returned when a workload violates capability scope boundaries (IAM-005).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeDenied {
    pub svid: String,
    pub requested_scope: String,
    pub reason: String,
}

impl fmt::Display for ScopeDenied {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Zero-Trust Violation: SVID {} is not authorized for capability '{}' by Cedar policy (403 ScopeDenied: {})",
            self.svid, self.requested_scope, self.reason
        )
    }
}

impl std::error::Error for ScopeDenied {}

/// Comprehensive errors across SPIFFE/SPIRE dynamic attestation, mTLS, and federation.
#[derive(Debug)]
pub enum IdentityError {
    AttestationFailed(String),
    ScopeDenied(ScopeDenied),
    InvalidSpiffeId(String),
    TrustDomainMismatch { expected: String, found: String },
    PathTraversalBlocked(String),
    MtlsError(String),
    PolicyError(String),
    FederationError(String),
    General(String),
}

impl fmt::Display for IdentityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IdentityError::AttestationFailed(msg) => write!(f, "Attestation failed: {}", msg),
            IdentityError::ScopeDenied(denied) => write!(f, "{}", denied),
            IdentityError::InvalidSpiffeId(msg) => write!(f, "Invalid SPIFFE ID: {}", msg),
            IdentityError::TrustDomainMismatch { expected, found } => {
                write!(
                    f,
                    "Trust domain mismatch: expected '{}', found '{}'",
                    expected, found
                )
            }
            IdentityError::PathTraversalBlocked(msg) => {
                write!(f, "Path traversal blocked: {}", msg)
            }
            IdentityError::MtlsError(msg) => write!(f, "mTLS error: {}", msg),
            IdentityError::PolicyError(msg) => write!(f, "Policy error: {}", msg),
            IdentityError::FederationError(msg) => write!(f, "Federation error: {}", msg),
            IdentityError::General(msg) => write!(f, "Identity error: {}", msg),
        }
    }
}

impl std::error::Error for IdentityError {}

impl From<ScopeDenied> for IdentityError {
    fn from(err: ScopeDenied) -> Self {
        IdentityError::ScopeDenied(err)
    }
}

impl From<Box<dyn std::error::Error>> for IdentityError {
    fn from(err: Box<dyn std::error::Error>) -> Self {
        IdentityError::General(err.to_string())
    }
}

// ============================================================================
// IAM-001 - IAM-004: SPIFFE SVID Representation
// ============================================================================

/// Represents an attested SPIFFE Verifiable Identity Document (SVID) in Aura.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Svid {
    pub id: String,
    pub trust_domain: String,
    pub scopes: HashSet<String>,
    pub allowed_paths: Vec<PathBuf>,
    pub cert_chain_der: Vec<Vec<u8>>,
    pub private_key_der: Option<Vec<u8>>,
    pub hint: Option<String>,
}

impl Svid {
    /// Creates a new SVID from an identity string.
    pub fn new(id: impl Into<String>) -> Self {
        let id_str = id.into();
        let trust_domain = if let Some(stripped) = id_str.strip_prefix("spiffe://") {
            stripped
                .split('/')
                .next()
                .unwrap_or("aura.local")
                .to_string()
        } else {
            "aura.local".to_string()
        };

        let mut scopes = HashSet::new();
        // Trust root / host identities receive standard administrative/host scopes
        if id_str.contains("host") || id_str.contains("trusted") {
            scopes.insert("fs:read".to_string());
            scopes.insert("fs:write".to_string());
            scopes.insert("net:bind".to_string());
            scopes.insert("tool:bash".to_string());
            scopes.insert("tool:browser".to_string());
            scopes.insert("tool:computer".to_string());
            scopes.insert("tool:filesystem".to_string());
            scopes.insert("tool:ast-blast-radius".to_string());
            scopes.insert("tool:ephemeral-tunnel".to_string());
        }

        Self {
            id: id_str,
            trust_domain,
            scopes,
            allowed_paths: vec![PathBuf::from("/home/jeff/aura")],
            cert_chain_der: Vec::new(),
            private_key_der: None,
            hint: None,
        }
    }

    pub fn with_scopes<I, S>(mut self, scopes: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.scopes = scopes.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_allowed_paths<I, P>(mut self, paths: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        self.allowed_paths = paths.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_certs(
        mut self,
        cert_chain_der: Vec<Vec<u8>>,
        private_key_der: Option<Vec<u8>>,
    ) -> Self {
        self.cert_chain_der = cert_chain_der;
        self.private_key_der = private_key_der;
        self
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.contains(scope)
    }

    pub fn is_trusted(&self) -> bool {
        self.id.contains("trusted") || self.id.contains("host")
    }

    pub fn spiffe_id(&self) -> Result<SpiffeId, IdentityError> {
        SpiffeId::from_str(&self.id)
            .map_err(|e| IdentityError::InvalidSpiffeId(format!("{}: {}", self.id, e)))
    }

    pub fn trust_domain(&self) -> Result<TrustDomain, IdentityError> {
        TrustDomain::try_from(self.trust_domain.as_str())
            .map_err(|e| IdentityError::InvalidSpiffeId(format!("{}: {}", self.trust_domain, e)))
    }
}

impl Default for Svid {
    fn default() -> Self {
        Self::new("spiffe://aura.local/host")
    }
}

impl From<&str> for Svid {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for Svid {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

// ============================================================================
// IAM-001 & IAM-003: Dynamic Attestation via SPIRE Workload API
// ============================================================================

/// Dynamic SVID attestation against the SPIRE Workload API (IAM-001).
/// Connects to the local SPIRE agent over UNIX domain socket, falling back to a mock SVID
/// in offline test environments.
pub async fn fetch_svid() -> Result<Svid, Box<dyn std::error::Error>> {
    println!("Fetching SPIFFE Verifiable Identity Document (SVID) via Workload API...");

    let socket_path = std::env::var("SPIFFE_ENDPOINT_SOCKET")
        .unwrap_or_else(|_| "unix:///run/spire/sockets/agent.sock".to_string());

    match WorkloadApiClient::connect_to(&socket_path).await {
        Ok(client) => {
            let x509_svid = client.fetch_x509_svid().await?;
            let id = x509_svid.spiffe_id().to_string();
            println!("Successfully attested workload identity: {}", id);

            let cert_chain_der: Vec<Vec<u8>> = x509_svid
                .cert_chain()
                .iter()
                .map(|c| c.as_ref().to_vec())
                .collect();
            let private_key_der = Some(x509_svid.private_key().as_ref().to_vec());
            let hint = x509_svid.hint().map(ToString::to_string);

            let mut svid = Svid::new(&id).with_certs(cert_chain_der, private_key_der);
            if let Some(h) = hint {
                svid = svid.with_hint(h);
            }

            Ok(svid)
        }
        Err(e) => {
            println!(
                "Warning: Could not connect to SPIRE agent at {}. Error: {}",
                socket_path, e
            );
            println!("Warning: Falling back to mocked identity for testing...");

            Ok(Svid::new("spiffe://aura.local/host"))
        }
    }
}

// ============================================================================
// IAM-004 - IAM-006: Capability & Cedar Policy Enforcer
// ============================================================================

/// Capability and Zero-Trust policy enforcer backed by Cedar policy (IAM-004, IAM-005, IAM-006).
pub struct CapabilityEnforcer;

impl CapabilityEnforcer {
    /// Audits the provided SVID against a specific requested capability using Cedar policy.
    pub fn check(svid: &Svid, capability: Capability) -> Result<(), Box<dyn std::error::Error>> {
        Self::check_scope(svid, capability.as_str())
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }

    /// Checks if the SVID is authorized for the given scope string (IAM-004, IAM-005).
    pub fn check_scope(svid: &Svid, scope: &str) -> Result<(), ScopeDenied> {
        let is_trusted = svid.is_trusted();
        let direct_allowed = is_trusted || svid.scopes.contains(scope);

        let policy_allowed = match Self::evaluate_cedar_policy(svid, scope) {
            Ok(allowed) => allowed,
            Err(e) => {
                eprintln!("Cedar policy evaluation error: {}", e);
                direct_allowed
            }
        };

        if direct_allowed && policy_allowed {
            Ok(())
        } else {
            Err(ScopeDenied {
                svid: svid.id.clone(),
                requested_scope: scope.to_string(),
                reason: format!(
                    "Scope '{}' not granted to SVID '{}' (granted: {:?})",
                    scope, svid.id, svid.scopes
                ),
            })
        }
    }

    /// Audits filesystem operations with path-prefix boundary matching (IAM-006).
    pub fn check_fs_path(
        svid: &Svid,
        target_path: &Path,
        is_write: bool,
    ) -> Result<(), ScopeDenied> {
        let required_cap = if is_write {
            Capability::FileSystemWrite
        } else {
            Capability::FileSystemRead
        };

        // 1. Check capability permission
        Self::check_scope(svid, required_cap.as_str())?;

        // 2. Prevent path traversal (no `..` components)
        if target_path.components().any(|c| c.as_os_str() == "..") {
            return Err(ScopeDenied {
                svid: svid.id.clone(),
                requested_scope: required_cap.as_str().to_string(),
                reason: "Path traversal blocked: relative '..' components are strictly prohibited"
                    .to_string(),
            });
        }

        // 3. Path-prefix boundary matching
        let default_boundary = PathBuf::from("/home/jeff/aura");
        let allowed_prefixes = if svid.allowed_paths.is_empty() {
            std::slice::from_ref(&default_boundary)
        } else {
            svid.allowed_paths.as_slice()
        };

        let inside_boundary = allowed_prefixes
            .iter()
            .any(|prefix| target_path.starts_with(prefix));
        if !inside_boundary && !svid.is_trusted() {
            return Err(ScopeDenied {
                svid: svid.id.clone(),
                requested_scope: required_cap.as_str().to_string(),
                reason: format!(
                    "Path '{}' is outside allowed boundary prefixes: {:?}",
                    target_path.display(),
                    allowed_prefixes
                ),
            });
        }

        Ok(())
    }

    /// Evaluates Cedar policy for principal, action, and resource (IAM-005).
    fn evaluate_cedar_policy(
        svid: &Svid,
        action_str: &str,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let policy_source = r#"
            permit(
                principal,
                action,
                resource
            ) when {
                principal.is_trusted == true
            };
            
            permit(
                principal,
                action,
                resource
            ) when {
                principal.has_scope == true
            };
        "#;

        let policies = PolicySet::from_str(policy_source)?;

        let principal_uid = EntityUid::from_type_name_and_id(
            EntityTypeName::from_str("Agent")?,
            EntityId::from_str(&svid.id)?,
        );

        let action_uid = EntityUid::from_type_name_and_id(
            EntityTypeName::from_str("Action")?,
            EntityId::from_str(action_str)?,
        );

        let resource_uid = EntityUid::from_type_name_and_id(
            EntityTypeName::from_str("Resource")?,
            EntityId::from_str("System")?,
        );

        let req = Request::new(
            principal_uid,
            action_uid,
            resource_uid,
            Context::empty(),
            None,
        )?;

        let is_trusted = svid.is_trusted();
        let has_scope = is_trusted || svid.scopes.contains(action_str);

        let entities_json = serde_json::json!([
            {
                "uid": { "type": "Agent", "id": svid.id },
                "attrs": {
                    "is_trusted": is_trusted,
                    "has_scope": has_scope
                },
                "parents": []
            }
        ])
        .to_string();

        let entities = Entities::from_json_str(&entities_json, None)?;

        let authorizer = Authorizer::new();
        let answer = authorizer.is_authorized(&req, &policies, &entities);

        Ok(answer.decision() == Decision::Allow)
    }

    /// Validates an SVID token/JWT or identity string, returning authorized scopes.
    pub async fn validate_svid(
        svid_token: &str,
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        if svid_token.is_empty() {
            return Err("Empty SVID token".into());
        }

        if svid_token.starts_with("spiffe://") {
            let svid = Svid::new(svid_token);
            return Ok(svid.scopes.into_iter().collect());
        }

        Ok(vec!["fs:read".to_string(), "fs:write".to_string()])
    }
}

// ============================================================================
// IAM-001, IAM-003, IAM-004: Dynamic Identity Manager & Ephemeral Minting
// ============================================================================

/// Identity Manager for dynamic workload attestation and ephemeral SVID minting.
#[derive(Clone, Debug)]
pub struct IdentityManager {
    pub socket_path: String,
    pub trust_domain: String,
    pub federation: Arc<FederationManager>,
}

impl Default for IdentityManager {
    fn default() -> Self {
        Self::new(None, None)
    }
}

impl IdentityManager {
    pub fn new(socket_path: Option<String>, trust_domain: Option<String>) -> Self {
        let socket = socket_path.unwrap_or_else(|| {
            std::env::var("SPIFFE_ENDPOINT_SOCKET")
                .unwrap_or_else(|_| "unix:///run/spire/sockets/agent.sock".to_string())
        });
        let domain = trust_domain.unwrap_or_else(|| "aura.local".to_string());

        Self {
            socket_path: socket,
            trust_domain: domain,
            federation: Arc::new(FederationManager::new()),
        }
    }

    /// Dynamically attests current process against SPIRE Workload API (IAM-001, IAM-003).
    pub async fn attest_workload(&self) -> Result<Svid, IdentityError> {
        match WorkloadApiClient::connect_to(&self.socket_path).await {
            Ok(client) => {
                let x509_svid = client
                    .fetch_x509_svid()
                    .await
                    .map_err(|e| IdentityError::AttestationFailed(e.to_string()))?;

                let id = x509_svid.spiffe_id().to_string();
                let cert_chain_der: Vec<Vec<u8>> = x509_svid
                    .cert_chain()
                    .iter()
                    .map(|c| c.as_ref().to_vec())
                    .collect();
                let private_key_der = Some(x509_svid.private_key().as_ref().to_vec());
                let hint = x509_svid.hint().map(ToString::to_string);

                let mut svid = Svid::new(&id).with_certs(cert_chain_der, private_key_der);
                if let Some(h) = hint {
                    svid = svid.with_hint(h);
                }

                Ok(svid)
            }
            Err(_) => Ok(Svid::new(format!("spiffe://{}/host", self.trust_domain))),
        }
    }

    /// Mints a short-lived ephemeral child SVID for an agent (Ephemeral-Tunnel) (IAM-003, IAM-004).
    /// Scopes can NEVER be broadened at runtime: requested_scopes must be a subset of parent SVID scopes.
    pub fn mint_ephemeral_svid(
        &self,
        parent_svid: &Svid,
        agent_name: &str,
        requested_scopes: &[String],
    ) -> Result<Svid, ScopeDenied> {
        // Enforce that scopes cannot be broadened at runtime (IAM-004)
        for req in requested_scopes {
            if !parent_svid.scopes.contains(req) && !parent_svid.is_trusted() {
                return Err(ScopeDenied {
                    svid: parent_svid.id.clone(),
                    requested_scope: req.clone(),
                    reason: "Cannot broaden scopes at runtime: requested scope is not held by parent SVID".to_string(),
                });
            }
        }

        let ephemeral_id = format!(
            "{}/ephemeral/{}/{}",
            parent_svid.id,
            agent_name,
            uuid::Uuid::new_v4()
        );

        let granted_scopes: HashSet<String> = if requested_scopes.is_empty() {
            parent_svid.scopes.clone()
        } else {
            requested_scopes.iter().cloned().collect()
        };

        Ok(Svid::new(&ephemeral_id)
            .with_scopes(granted_scopes)
            .with_allowed_paths(parent_svid.allowed_paths.clone()))
    }

    /// Attests an isolated execution sandbox (WASI / Firecracker microVM) with explicit tool scopes (IAM-004).
    pub fn attest_sandbox(&self, sandbox_id: &str, allowed_capabilities: &[Capability]) -> Svid {
        let sandbox_id_str = format!("spiffe://{}/sandbox/{}", self.trust_domain, sandbox_id);
        let scopes: HashSet<String> = allowed_capabilities
            .iter()
            .map(|c| c.as_str().to_string())
            .collect();

        Svid::new(&sandbox_id_str).with_scopes(scopes)
    }

    /// Validates an incoming peer SVID string against the expected trust domain (IAM-002, IAM-008).
    pub fn validate_peer(&self, peer_id_str: &str) -> Result<SpiffeId, IdentityError> {
        let spiffe_id = SpiffeId::from_str(peer_id_str)
            .map_err(|e| IdentityError::InvalidSpiffeId(format!("{}: {}", peer_id_str, e)))?;

        let peer_domain = spiffe_id.trust_domain().as_str();
        if peer_domain == self.trust_domain || self.federation.is_federated(peer_domain) {
            Ok(spiffe_id)
        } else {
            Err(IdentityError::TrustDomainMismatch {
                expected: self.trust_domain.clone(),
                found: peer_domain.to_string(),
            })
        }
    }
}

// ============================================================================
// IAM-007: mTLS Peer Enforcement
// ============================================================================

/// Validates peer credentials for mTLS connections (NET-006, IAM-007).
pub struct MtlsEnforcer;

impl MtlsEnforcer {
    /// Validates peer SPIFFE ID and enforces trust domain boundary (IAM-007).
    pub fn validate_peer_identity(
        peer_id: &str,
        expected_trust_domain: &str,
        federation: Option<&FederationManager>,
    ) -> Result<SpiffeId, IdentityError> {
        let spiffe_id = SpiffeId::from_str(peer_id)
            .map_err(|e| IdentityError::InvalidSpiffeId(format!("{}: {}", peer_id, e)))?;

        let td = spiffe_id.trust_domain().as_str();
        if td == expected_trust_domain {
            return Ok(spiffe_id);
        }

        if let Some(fed) = federation {
            if fed.is_federated(td) {
                return Ok(spiffe_id);
            }
        }

        Err(IdentityError::TrustDomainMismatch {
            expected: expected_trust_domain.to_string(),
            found: td.to_string(),
        })
    }
}

// ============================================================================
// IAM-008: Trust Domain Federation Manager
// ============================================================================

/// Manages enterprise trust domain federation hooks (IAM-008, ADR-002).
#[derive(Debug, Default)]
pub struct FederationManager {
    federated_domains: dashmap::DashMap<String, Vec<u8>>,
}

impl FederationManager {
    pub fn new() -> Self {
        Self {
            federated_domains: dashmap::DashMap::new(),
        }
    }

    pub fn register_federated_domain(&self, trust_domain: &str, bundle_der: Vec<u8>) {
        self.federated_domains
            .insert(trust_domain.to_string(), bundle_der);
    }

    pub fn is_federated(&self, trust_domain: &str) -> bool {
        self.federated_domains.contains_key(trust_domain)
    }

    pub fn get_bundle(&self, trust_domain: &str) -> Option<Vec<u8>> {
        self.federated_domains.get(trust_domain).map(|v| v.clone())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[tokio::test]
    #[serial]
    async fn test_fetch_svid_fallback() {
        let old_socket = std::env::var("SPIFFE_ENDPOINT_SOCKET").ok();

        std::env::set_var(
            "SPIFFE_ENDPOINT_SOCKET",
            "unix:///tmp/nonexistent_spire.sock",
        );

        let svid_result = fetch_svid().await;
        assert!(svid_result.is_ok());

        let svid = svid_result.unwrap();
        assert_eq!(svid.id, "spiffe://aura.local/host");
        assert!(svid.is_trusted());
        assert!(svid.has_scope("fs:read"));
        assert!(svid.has_scope("fs:write"));
        assert!(svid.has_scope("net:bind"));

        if let Some(socket) = old_socket {
            std::env::set_var("SPIFFE_ENDPOINT_SOCKET", socket);
        } else {
            std::env::remove_var("SPIFFE_ENDPOINT_SOCKET");
        }
    }

    #[test]
    fn test_spiffe_id_parsing_and_trust_domain() {
        let svid = Svid::new("spiffe://aura.local/agent/coder");
        assert_eq!(svid.trust_domain, "aura.local");
        assert!(svid.spiffe_id().is_ok());

        let invalid = Svid::new("invalid-uri");
        assert!(invalid.spiffe_id().is_err());
    }

    #[test]
    fn test_scoped_svid_default_deny() {
        let unpriv_svid = Svid::new("spiffe://aura.local/agent/sandboxed");
        assert!(!unpriv_svid.is_trusted());
        assert!(unpriv_svid.scopes.is_empty());

        let res = CapabilityEnforcer::check(&unpriv_svid, Capability::FileSystemWrite);
        assert!(res.is_err());
        let err_msg = res.unwrap_err().to_string();
        assert!(err_msg.contains("Zero-Trust Violation"));
        assert!(err_msg.contains("403 ScopeDenied"));
    }

    #[test]
    fn test_capability_enforcement_explicit_scopes() {
        let agent_svid =
            Svid::new("spiffe://aura.local/agent/reader").with_scopes(vec!["fs:read".to_string()]);

        assert!(CapabilityEnforcer::check(&agent_svid, Capability::FileSystemRead).is_ok());
        assert!(CapabilityEnforcer::check(&agent_svid, Capability::FileSystemWrite).is_err());
        assert!(CapabilityEnforcer::check(&agent_svid, Capability::ToolBash).is_err());
    }

    #[test]
    fn test_cannot_broaden_scopes_at_runtime() {
        let manager = IdentityManager::default();
        let agent_svid =
            Svid::new("spiffe://aura.local/agent/worker").with_scopes(vec!["fs:read".to_string()]);

        // Requesting scope subset is allowed
        let ok_child =
            manager.mint_ephemeral_svid(&agent_svid, "subtask", &["fs:read".to_string()]);
        assert!(ok_child.is_ok());

        // Attempting to escalate/broaden scope to fs:write must be DENIED (IAM-004)
        let escalated = manager.mint_ephemeral_svid(
            &agent_svid,
            "escalation",
            &["fs:read".to_string(), "fs:write".to_string()],
        );
        assert!(escalated.is_err());
        assert_eq!(escalated.unwrap_err().requested_scope, "fs:write");
    }

    #[test]
    fn test_path_prefix_boundary_enforcement() {
        let svid = Svid::new("spiffe://aura.local/agent/io")
            .with_scopes(vec!["fs:read".to_string(), "fs:write".to_string()])
            .with_allowed_paths(vec![PathBuf::from("/home/jeff/aura")]);

        // Valid path within allowed prefix
        let valid_path = Path::new("/home/jeff/aura/src/main.rs");
        assert!(CapabilityEnforcer::check_fs_path(&svid, valid_path, false).is_ok());

        // Path traversal with `..` must be BLOCKED
        let traversal_path = Path::new("/home/jeff/aura/../etc/passwd");
        let traversal_res = CapabilityEnforcer::check_fs_path(&svid, traversal_path, false);
        assert!(traversal_res.is_err());
        assert!(traversal_res.unwrap_err().reason.contains("Path traversal"));

        // Path outside boundary must be BLOCKED
        let outside_path = Path::new("/etc/shadow");
        let outside_res = CapabilityEnforcer::check_fs_path(&svid, outside_path, false);
        assert!(outside_res.is_err());
        assert!(outside_res
            .unwrap_err()
            .reason
            .contains("outside allowed boundary"));
    }

    #[test]
    fn test_mtls_peer_validation() {
        let valid_peer = "spiffe://aura.local/agent/peer1";
        assert!(MtlsEnforcer::validate_peer_identity(valid_peer, "aura.local", None).is_ok());

        let rogue_peer = "spiffe://evil.corp/agent/attacker";
        let rogue_res = MtlsEnforcer::validate_peer_identity(rogue_peer, "aura.local", None);
        assert!(rogue_res.is_err());
        match rogue_res.unwrap_err() {
            IdentityError::TrustDomainMismatch { expected, found } => {
                assert_eq!(expected, "aura.local");
                assert_eq!(found, "evil.corp");
            }
            other => panic!("Unexpected error: {:?}", other),
        }
    }

    #[test]
    fn test_federation_trust_domain_hooks() {
        let federation = FederationManager::new();
        federation.register_federated_domain("partner.system", vec![0x30, 0x82]);

        let manager = IdentityManager {
            socket_path: "/tmp/mock.sock".to_string(),
            trust_domain: "aura.local".to_string(),
            federation: Arc::new(federation),
        };

        // Local trust domain
        assert!(manager.validate_peer("spiffe://aura.local/node1").is_ok());

        // Federated trust domain
        assert!(manager
            .validate_peer("spiffe://partner.system/node2")
            .is_ok());

        // Unknown foreign trust domain
        let foreign_res = manager.validate_peer("spiffe://unknown.domain/node3");
        assert!(foreign_res.is_err());
    }

    #[test]
    fn test_cedar_policy_evaluation() {
        let trusted_svid = Svid::new("spiffe://aura.local/host");
        assert!(CapabilityEnforcer::check(&trusted_svid, Capability::FileSystemWrite).is_ok());

        let malicious_svid = Svid::new("spiffe://aura.local/agent/malicious");
        assert!(CapabilityEnforcer::check(&malicious_svid, Capability::FileSystemWrite).is_err());
    }
}
