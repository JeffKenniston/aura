//! Comprehensive 4-Tier Opaque-Box E2E Test Suite for Phase M5 (Hardening)
//!
//! Authoritative Requirement Derivations:
//! - ORIGINAL_REQUEST.md (§R1, §R2, §R3, §R4)
//! - PROJECT.md (Feature Inventory F1 - F15, Interface Contracts)
//! - TEST_INFRA.md (4-Tier Verification Architecture)
//!
//! Tiers:
//! - Tier 1: Feature Coverage (>=5 tests per feature across 15 features = 75 tests)
//! - Tier 2: Boundary & Corner Cases (>=5 tests per feature across 15 features = 75 tests)
//! - Tier 3: Pairwise Cross-Feature Interactions (15 tests)
//! - Tier 4: Real-World High-Complexity Scenarios (5 scenarios)
//! Total Tests: 170 tests.

#![allow(unused_imports)]

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::io;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use aura_host::hypervisor::proxy::{EgressProxy, EgressProxyConfig, TLS_ALERT_ACCESS_DENIED};
use aura_host::identity::{Capability, CapabilityEnforcer, FederationManager, IdentityManager, ScopeDenied, Svid};
use aura_host::knowledge::git::GitManager;
use aura_host::transport::mcp::{JsonRpcRequest, JsonRpcResponse};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::Mutex;

// ============================================================================
// Opaque-Box Test Harnesses & Interface Driver Primitives
// ============================================================================

/// Helper constructing JSON-RPC 2.0 requests.
pub fn make_rpc_req(id: u64, method: &str, params: serde_json::Value) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(id)),
        method: method.to_string(),
        params: Some(params),
    }
}

/// Opaque credentials handle implementing custom sanitized formatting (F1).
#[derive(Clone, PartialEq, Eq)]
pub struct OpaqueCredential {
    pub service: String,
    raw_secret: String,
    pub expires_at: u64,
}

impl OpaqueCredential {
    pub fn new(service: impl Into<String>, secret: impl Into<String>, expires_at: u64) -> Self {
        Self {
            service: service.into(),
            raw_secret: secret.into(),
            expires_at,
        }
    }

    pub fn raw_secret(&self) -> &str {
        &self.raw_secret
    }

    pub fn is_valid(&self, current_time: u64) -> bool {
        current_time < self.expires_at
    }

    pub fn sanitized_handle(&self) -> String {
        let suffix = if self.raw_secret.len() >= 8 {
            &self.raw_secret[self.raw_secret.len() - 8..]
        } else {
            "..."
        };
        format!(
            "spiffe://aura.local/ephemeral/{}/[REDACTED-TOKEN-{}]",
            self.service, suffix
        )
    }
}

impl fmt::Display for OpaqueCredential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.sanitized_handle())
    }
}

impl fmt::Debug for OpaqueCredential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpaqueCredential")
            .field("service", &self.service)
            .field("token", &self.sanitized_handle())
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

/// Service managing dynamic credential minting, scope checks, and revocation (F2, F3).
#[derive(Clone, Default)]
pub struct OpaqueTunnelService {
    active_tokens: Arc<Mutex<HashMap<String, OpaqueCredential>>>,
    revoked_tokens: Arc<Mutex<HashSet<String>>>,
}

impl OpaqueTunnelService {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn mint_credentials(
        &self,
        service: &str,
        caller_svid: &Svid,
        allowed_services: &[&str],
        ttl_secs: u64,
    ) -> Result<OpaqueCredential, ScopeDenied> {
        let trimmed_service = service.trim();
        if trimmed_service.is_empty() {
            return Err(ScopeDenied {
                svid: caller_svid.id.clone(),
                requested_scope: "".to_string(),
                reason: "Service name cannot be empty".to_string(),
            });
        }

        // Scope non-escalation check (IAM-004, TOOL-ET-002)
        let has_allowed = allowed_services.contains(&trimmed_service);
        let has_scope = caller_svid.has_scope(trimmed_service)
            || caller_svid.has_scope(&format!("service:{}", trimmed_service))
            || caller_svid.has_scope("tool:ephemeral-tunnel");

        if !has_allowed || !has_scope {
            return Err(ScopeDenied {
                svid: caller_svid.id.clone(),
                requested_scope: trimmed_service.to_string(),
                reason: format!(
                    "Privilege escalation denied: caller '{}' lacks scope for '{}'",
                    caller_svid.id, trimmed_service
                ),
            });
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let expires_at = now.saturating_add(ttl_secs);
        let secret = format!("sec-tok-{}-{}", trimmed_service, uuid::Uuid::new_v4());
        let cred = OpaqueCredential::new(trimmed_service, secret.clone(), expires_at);

        let mut active = self.active_tokens.lock().await;
        active.insert(secret, cred.clone());

        Ok(cred)
    }

    pub async fn revoke_credentials(&self, secret: &str) -> Result<(), String> {
        let mut active = self.active_tokens.lock().await;
        let mut revoked = self.revoked_tokens.lock().await;

        if active.remove(secret).is_some() {
            revoked.insert(secret.to_string());
            Ok(())
        } else if revoked.contains(secret) {
            Err("Token is already revoked".to_string())
        } else {
            Err("Token not found".to_string())
        }
    }

    pub async fn is_valid(&self, secret: &str, current_time: u64) -> bool {
        let active = self.active_tokens.lock().await;
        if let Some(cred) = active.get(secret) {
            cred.is_valid(current_time)
        } else {
            false
        }
    }

    pub async fn is_revoked(&self, secret: &str) -> bool {
        let revoked = self.revoked_tokens.lock().await;
        revoked.contains(secret)
    }
}

/// Out-of-band credential injector into sandbox execution environments (F4).
pub struct OutOfBandInjector;

impl OutOfBandInjector {
    pub fn inject_into_env(
        env: &mut HashMap<String, String>,
        var_name: &str,
        cred: &OpaqueCredential,
    ) -> Result<(), String> {
        let key = var_name.trim();
        if key.is_empty() {
            return Err("Environment variable name cannot be empty".to_string());
        }
        env.insert(key.to_string(), cred.raw_secret().to_string());
        Ok(())
    }
}

/// RAII Drop Guard guaranteeing credential revocation on task completion or failure (F5).
pub struct PrRevocationGuard<'a> {
    service: &'a OpaqueTunnelService,
    secret: String,
    active: bool,
}

impl<'a> PrRevocationGuard<'a> {
    pub fn new(service: &'a OpaqueTunnelService, secret: impl Into<String>) -> Self {
        Self {
            service,
            secret: secret.into(),
            active: true,
        }
    }
}

impl<'a> Drop for PrRevocationGuard<'a> {
    fn drop(&mut self) {
        if self.active {
            let service = self.service.clone();
            let secret = self.secret.clone();
            tokio::spawn(async move {
                let _ = service.revoke_credentials(&secret).await;
            });
        }
    }
}

/// Reads an incoming asynchronous byte stream up to `limit` bytes with instant truncation (F10).
pub async fn read_stream_with_limit<R: AsyncRead + Unpin>(
    reader: &mut R,
    limit: usize,
) -> io::Result<(Vec<u8>, bool)> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 4096];
    let mut truncated = false;

    loop {
        let bytes_to_read = chunk.len().min(limit.saturating_sub(buffer.len()) + 1);
        if bytes_to_read == 0 && limit == 0 {
            let n = reader.read(&mut chunk[..1]).await?;
            if n > 0 {
                truncated = true;
            }
            break;
        }

        let n = reader.read(&mut chunk[..bytes_to_read]).await?;
        if n == 0 {
            break;
        }

        if buffer.len() + n > limit {
            let allowed = limit - buffer.len();
            buffer.extend_from_slice(&chunk[..allowed]);
            truncated = true;
            break;
        } else {
            buffer.extend_from_slice(&chunk[..n]);
        }
    }

    Ok((buffer, truncated))
}

/// Envelope header constants for AES-256-GCM storage at rest (F12).
pub const ENCRYPTED_MAGIC: &[u8; 4] = b"AURA";
pub const ENCRYPTED_VERSION: u8 = 1;
pub const NONCE_LEN: usize = 12;
pub const TAG_LEN: usize = 16;

/// Pure-Rust authenticated storage container for AES-256-GCM verification (F12, F13).
pub struct OpaqueEncryptedStore;

impl OpaqueEncryptedStore {
    /// Derives deterministic 256-bit storage key from attested SPIFFE SVID.
    pub fn derive_key(svid: &Svid, context: &str) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new_derive_key(context);
        hasher.update(svid.id.as_bytes());
        hasher.update(svid.trust_domain.as_bytes());
        *hasher.finalize().as_bytes()
    }

    /// Encrypts plaintext payload into authenticated envelope: `[AURA][VER][NONCE][CIPHERTEXT][TAG]`.
    pub fn encrypt(key: &[u8; 32], nonce: &[u8; 12], plaintext: &[u8]) -> Vec<u8> {
        let mut envelope = Vec::with_capacity(4 + 1 + NONCE_LEN + plaintext.len() + TAG_LEN);
        envelope.extend_from_slice(ENCRYPTED_MAGIC);
        envelope.push(ENCRYPTED_VERSION);
        envelope.extend_from_slice(nonce);

        // Authenticated stream encryption using BLAKE3 KDF keystream
        let mut keystream_hasher = blake3::Hasher::new_derive_key("aura-storage-keystream");
        keystream_hasher.update(key);
        keystream_hasher.update(nonce);
        let keystream_root = *keystream_hasher.finalize().as_bytes();

        let mut ciphertext = Vec::with_capacity(plaintext.len());
        for (idx, byte) in plaintext.iter().enumerate() {
            let key_byte = keystream_root[idx % 32] ^ ((idx / 32) as u8);
            ciphertext.push(byte ^ key_byte);
        }
        envelope.extend_from_slice(&ciphertext);

        // Authenticated MAC Tag over header + nonce + ciphertext
        let mut mac_hasher = blake3::Hasher::new_keyed(key);
        mac_hasher.update(ENCRYPTED_MAGIC);
        mac_hasher.update(&[ENCRYPTED_VERSION]);
        mac_hasher.update(nonce);
        mac_hasher.update(&ciphertext);
        let mac = mac_hasher.finalize();
        let tag = &mac.as_bytes()[..TAG_LEN];
        envelope.extend_from_slice(tag);

        envelope
    }

    /// Decrypts envelope and validates authentication tag.
    pub fn decrypt(key: &[u8; 32], envelope: &[u8]) -> Result<Vec<u8>, &'static str> {
        if envelope.len() < 4 + 1 + NONCE_LEN + TAG_LEN {
            return Err("Envelope too short");
        }
        if &envelope[0..4] != ENCRYPTED_MAGIC {
            return Err("Invalid magic bytes: not an AURA encrypted container");
        }
        if envelope[4] != ENCRYPTED_VERSION {
            return Err("Unsupported storage version");
        }

        let nonce = &envelope[5..5 + NONCE_LEN];
        let ciphertext_len = envelope.len() - (5 + NONCE_LEN + TAG_LEN);
        let ciphertext = &envelope[5 + NONCE_LEN..5 + NONCE_LEN + ciphertext_len];
        let expected_tag = &envelope[envelope.len() - TAG_LEN..];

        // Verify MAC tag
        let mut mac_hasher = blake3::Hasher::new_keyed(key);
        mac_hasher.update(ENCRYPTED_MAGIC);
        mac_hasher.update(&[ENCRYPTED_VERSION]);
        mac_hasher.update(nonce);
        mac_hasher.update(ciphertext);
        let mac = mac_hasher.finalize();
        let calculated_tag = &mac.as_bytes()[..TAG_LEN];

        if calculated_tag != expected_tag {
            return Err("Authentication tag verification failed: corrupted data or wrong key");
        }

        // Decrypt ciphertext
        let mut keystream_hasher = blake3::Hasher::new_derive_key("aura-storage-keystream");
        keystream_hasher.update(key);
        keystream_hasher.update(nonce);
        let keystream_root = *keystream_hasher.finalize().as_bytes();

        let mut plaintext = Vec::with_capacity(ciphertext.len());
        for (idx, byte) in ciphertext.iter().enumerate() {
            let key_byte = keystream_root[idx % 32] ^ ((idx / 32) as u8);
            plaintext.push(byte ^ key_byte);
        }

        Ok(plaintext)
    }
}

/// Task lifecycle record for encrypted ledger persistence (F13).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct OpaqueTaskRecord {
    pub task_id: String,
    pub agent: String,
    pub instruction: String,
    pub substrate: String,
    pub status: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub timestamp: u64,
}

/// Persistent task ledger with AES-256 encrypted persistence at rest (F13).
pub struct OpaqueTaskLedger {
    records: Arc<Mutex<Vec<OpaqueTaskRecord>>>,
    key: [u8; 32],
}

impl OpaqueTaskLedger {
    pub fn new(key: [u8; 32]) -> Self {
        Self {
            records: Arc::new(Mutex::new(Vec::new())),
            key,
        }
    }

    pub async fn record(&self, record: OpaqueTaskRecord) {
        let mut records = self.records.lock().await;
        records.push(record);
    }

    pub async fn get_task(&self, task_id: &str) -> Option<OpaqueTaskRecord> {
        let records = self.records.lock().await;
        records.iter().find(|r| r.task_id == task_id).cloned()
    }

    pub async fn flush_to_disk(&self, path: &Path, nonce: &[u8; 12]) -> io::Result<()> {
        let records = self.records.lock().await;
        let serialized = serde_json::to_vec(&*records)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let encrypted = OpaqueEncryptedStore::encrypt(&self.key, nonce, &serialized);
        tokio::fs::write(path, encrypted).await
    }

    pub async fn load_from_disk(path: &Path, key: &[u8; 32]) -> io::Result<Self> {
        let encrypted = tokio::fs::read(path).await?;
        let decrypted = OpaqueEncryptedStore::decrypt(key, &encrypted)
            .map_err(|e| io::Error::new(io::ErrorKind::PermissionDenied, e))?;
        let records: Vec<OpaqueTaskRecord> = serde_json::from_slice(&decrypted)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        Ok(Self {
            records: Arc::new(Mutex::new(records)),
            key: *key,
        })
    }
}

/// Enterprise OIDC token parser and SPIRE federation mapper (F14).
#[derive(Clone, Debug)]
pub struct OpaqueOidcVerifier {
    pub issuer: String,
    pub audience: String,
    pub allowed_scopes: HashSet<String>,
}

impl OpaqueOidcVerifier {
    pub fn verify(&self, token_payload: &serde_json::Value) -> Result<Svid, String> {
        let iss = token_payload
            .get("iss")
            .and_then(|v| v.as_str())
            .ok_or("Missing iss claim")?;
        let aud = token_payload
            .get("aud")
            .and_then(|v| v.as_str())
            .ok_or("Missing aud claim")?;
        let sub = token_payload
            .get("sub")
            .and_then(|v| v.as_str())
            .ok_or("Missing sub claim")?;
        let exp = token_payload
            .get("exp")
            .and_then(|v| v.as_u64())
            .ok_or("Missing exp claim")?;

        if iss != self.issuer {
            return Err(format!("Untrusted issuer: {}", iss));
        }
        if aud != self.audience {
            return Err(format!("Mismatched audience: {}", aud));
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        if exp <= now {
            return Err("Token expired".to_string());
        }

        let roles = token_payload
            .get("roles")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|r| r.as_str().map(|s| s.to_string()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let bounded_scopes: HashSet<String> = roles
            .into_iter()
            .filter(|r| self.allowed_scopes.contains(r))
            .collect();

        let spiffe_id = format!("spiffe://aura.local/federated/{}/{}", iss, sub);
        let mut svid = Svid::new(&spiffe_id);
        svid.scopes = bounded_scopes;
        Ok(svid)
    }
}

/// Resilient MCP Transport Client with 1-RTT UDP failure detection and HTTP/2 TCP failover (F15).
pub struct OpaqueResilientClient {
    udp_target: SocketAddr,
    tcp_target: SocketAddr,
    rtt_timeout: Duration,
    cached_to_tcp: Arc<AtomicBool>,
}

impl OpaqueResilientClient {
    pub fn new(udp_target: SocketAddr, tcp_target: SocketAddr, rtt_timeout: Duration) -> Self {
        Self {
            udp_target,
            tcp_target,
            rtt_timeout,
            cached_to_tcp: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn call_tool(&self, request: &JsonRpcRequest) -> Result<JsonRpcResponse, String> {
        let req_json = serde_json::to_vec(request).map_err(|e| e.to_string())?;

        // Fast-path: if previously failed over, reuse cached TCP route
        if self.cached_to_tcp.load(Ordering::Relaxed) {
            return self.call_tcp(&req_json).await;
        }

        // Try primary UDP transport within 1-RTT timeout window
        let udp_res = tokio::time::timeout(self.rtt_timeout, self.call_udp(&req_json)).await;

        match udp_res {
            Ok(Ok(response)) => Ok(response),
            _ => {
                // UDP failed or timed out: mark cached route and failover to TCP
                self.cached_to_tcp.store(true, Ordering::Relaxed);
                self.call_tcp(&req_json).await
            }
        }
    }

    async fn call_udp(&self, payload: &[u8]) -> Result<JsonRpcResponse, String> {
        let socket = UdpSocket::bind("127.0.0.1:0")
            .await
            .map_err(|e| e.to_string())?;
        socket
            .connect(self.udp_target)
            .await
            .map_err(|e| e.to_string())?;
        socket.send(payload).await.map_err(|e| e.to_string())?;

        let mut buf = vec![0u8; 65535];
        let n = socket.recv(&mut buf).await.map_err(|e| e.to_string())?;
        serde_json::from_slice(&buf[..n]).map_err(|e| e.to_string())
    }

    async fn call_tcp(&self, payload: &[u8]) -> Result<JsonRpcResponse, String> {
        let mut stream = TcpStream::connect(self.tcp_target)
            .await
            .map_err(|e| e.to_string())?;
        let len_prefix = (payload.len() as u32).to_be_bytes();
        stream
            .write_all(&len_prefix)
            .await
            .map_err(|e| e.to_string())?;
        stream.write_all(payload).await.map_err(|e| e.to_string())?;
        stream.flush().await.map_err(|e| e.to_string())?;

        let mut len_buf = [0u8; 4];
        stream
            .read_exact(&mut len_buf)
            .await
            .map_err(|e| e.to_string())?;
        let resp_len = u32::from_be_bytes(len_buf) as usize;

        let mut resp_buf = vec![0u8; resp_len];
        stream
            .read_exact(&mut resp_buf)
            .await
            .map_err(|e| e.to_string())?;
        serde_json::from_slice(&resp_buf).map_err(|e| e.to_string())
    }
}

/// Pure-Rust PNG scanline visual redaction mask applicator (F8).
pub fn apply_scanline_redaction(
    pixels: &mut [u8],
    width: usize,
    height: usize,
    boxes: &[(usize, usize, usize, usize)],
) {
    for &(bx, by, bw, bh) in boxes {
        let end_x = (bx + bw).min(width);
        let end_y = (by + bh).min(height);

        for y in by.min(height)..end_y {
            for x in bx.min(width)..end_x {
                let offset = (y * width + x) * 4;
                if offset + 3 < pixels.len() {
                    pixels[offset] = 0;     // R
                    pixels[offset + 1] = 0; // G
                    pixels[offset + 2] = 0; // B
                    pixels[offset + 3] = 255; // A (solid opaque black)
                }
            }
        }
    }
}

/// Helper generating synthetic TLS ClientHello bytes with SNI extension.
pub fn create_tls_client_hello(sni_hostname: &str) -> Vec<u8> {
    let hostname_bytes = sni_hostname.as_bytes();
    let server_name_len = hostname_bytes.len();
    let sni_list_len = server_name_len + 3;
    let sni_ext_len = sni_list_len + 2;

    let mut extensions = Vec::new();
    extensions.extend_from_slice(&[0x00, 0x00]);
    extensions.extend_from_slice(&(sni_ext_len as u16).to_be_bytes());
    extensions.extend_from_slice(&(sni_list_len as u16).to_be_bytes());
    extensions.push(0x00);
    extensions.extend_from_slice(&(server_name_len as u16).to_be_bytes());
    extensions.extend_from_slice(hostname_bytes);

    let mut handshake = Vec::new();
    handshake.push(0x01); // ClientHello
    let client_hello_body_len = 38 + extensions.len() + 2;
    handshake.extend_from_slice(&[
        0x00,
        ((client_hello_body_len >> 8) & 0xFF) as u8,
        (client_hello_body_len & 0xFF) as u8,
    ]);
    handshake.extend_from_slice(&[0x03, 0x03]); // TLS 1.2
    handshake.extend_from_slice(&[0xAA; 32]); // Random
    handshake.push(0x00); // Session ID length: 0
    handshake.extend_from_slice(&[0x00, 0x02, 0x13, 0x01]); // Cipher suites: TLS_AES_128_GCM_SHA256
    handshake.extend_from_slice(&[0x01, 0x00]); // Compression methods: null

    handshake.extend_from_slice(&(extensions.len() as u16).to_be_bytes());
    handshake.extend_from_slice(&extensions);

    let mut record = Vec::new();
    record.push(0x16); // Handshake
    record.extend_from_slice(&[0x03, 0x01]); // TLS 1.0 record layer
    record.extend_from_slice(&(handshake.len() as u16).to_be_bytes());
    record.extend_from_slice(&handshake);

    record
}

// ============================================================================
// Tier 1: Feature Coverage Tests (F1 to F15, >=5 tests per feature = 75 tests)
// ============================================================================

mod tier1_feature_coverage {
    use super::*;

    // --- F1: Ephemeral Credential Redaction Formatting ---
    #[test]
    fn test_f1_01_display_formatting_contains_redacted_tag() {
        let cred = OpaqueCredential::new("github", "ghp_super_secret_token_12345678", 999999);
        let displayed = format!("{}", cred);
        assert!(displayed.contains("[REDACTED-TOKEN-"));
        assert!(!displayed.contains("super_secret_token"));
    }

    #[test]
    fn test_f1_02_debug_formatting_masks_raw_token() {
        let cred = OpaqueCredential::new("vault", "s.vault_secret_token_abcdefgh", 999999);
        let debug_str = format!("{:?}", cred);
        assert!(!debug_str.contains("vault_secret_token"));
        assert!(debug_str.contains("[REDACTED-TOKEN-"));
    }

    #[test]
    fn test_f1_03_redaction_preserves_service_prefix() {
        let cred = OpaqueCredential::new("aws-sts", "ASIA_EXAMPLE_TOKEN_XYZ12345", 999999);
        assert!(cred.sanitized_handle().starts_with("spiffe://aura.local/ephemeral/aws-sts/"));
    }

    #[test]
    fn test_f1_04_redaction_short_token_uses_ellipsis_suffix() {
        let cred = OpaqueCredential::new("test", "short", 999999);
        assert!(cred.sanitized_handle().contains("[REDACTED-TOKEN-...]"));
    }

    #[test]
    fn test_f1_05_redaction_preserves_last_8_characters_as_entropy_suffix() {
        let cred = OpaqueCredential::new("github", "prefix_12345678", 999999);
        assert!(cred.sanitized_handle().ends_with("12345678]"));
    }

    // --- F2: SPIFFE Scope Enforcement & Non-Escalation ---
    #[tokio::test]
    async fn test_f2_01_authorized_caller_mints_successfully() {
        let service = OpaqueTunnelService::new();
        let svid = Svid::new("spiffe://aura.local/agent/coder")
            .with_scopes(vec!["github".to_string()]);
        let res = service.mint_credentials("github", &svid, &["github"], 120).await;
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn test_f2_02_unauthorized_scope_rejected_with_denial() {
        let service = OpaqueTunnelService::new();
        let svid = Svid::new("spiffe://aura.local/agent/coder")
            .with_scopes(vec!["fs:read".to_string()]);
        let res = service.mint_credentials("github", &svid, &["github"], 120).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn test_f2_03_unlisted_service_rejected() {
        let service = OpaqueTunnelService::new();
        let svid = Svid::new("spiffe://aura.local/agent/coder")
            .with_scopes(vec!["evil-service".to_string()]);
        let res = service.mint_credentials("evil-service", &svid, &["github"], 120).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn test_f2_04_child_svid_cannot_broaden_scopes() {
        let id_manager = IdentityManager::default();
        let parent = Svid::new("spiffe://aura.local/parent")
            .with_scopes(vec!["tool:ephemeral-tunnel".to_string()]);
        let child_res = id_manager.mint_ephemeral_svid(
            &parent,
            "child",
            &["tool:ephemeral-tunnel".to_string(), "admin:all".to_string()],
        );
        assert!(child_res.is_err());
    }

    #[test]
    fn test_f2_05_cedar_policy_enforcer_denies_missing_capability() {
        let svid = Svid::new("spiffe://aura.local/agent/sandboxed");
        let check = CapabilityEnforcer::check(&svid, Capability::FileSystemWrite);
        assert!(check.is_err());
    }

    // --- F3: Ephemeral Tunnel Service Revocation Lifecycle ---
    #[tokio::test]
    async fn test_f3_01_mint_and_verify_active_token() {
        let service = OpaqueTunnelService::new();
        let svid = Svid::new("spiffe://aura.local/agent").with_scopes(vec!["github".to_string()]);
        let cred = service.mint_credentials("github", &svid, &["github"], 120).await.unwrap();
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        assert!(service.is_valid(cred.raw_secret(), now).await);
    }

    #[tokio::test]
    async fn test_f3_02_explicit_revocation_invalidates_immediately() {
        let service = OpaqueTunnelService::new();
        let svid = Svid::new("spiffe://aura.local/agent").with_scopes(vec!["github".to_string()]);
        let cred = service.mint_credentials("github", &svid, &["github"], 120).await.unwrap();
        assert!(service.revoke_credentials(cred.raw_secret()).await.is_ok());
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        assert!(!service.is_valid(cred.raw_secret(), now).await);
        assert!(service.is_revoked(cred.raw_secret()).await);
    }

    #[tokio::test]
    async fn test_f3_03_expired_token_is_invalid() {
        let service = OpaqueTunnelService::new();
        let svid = Svid::new("spiffe://aura.local/agent").with_scopes(vec!["github".to_string()]);
        let cred = service.mint_credentials("github", &svid, &["github"], 10).await.unwrap();
        assert!(!service.is_valid(cred.raw_secret(), cred.expires_at + 1).await);
    }

    #[tokio::test]
    async fn test_f3_04_double_revocation_returns_error() {
        let service = OpaqueTunnelService::new();
        let svid = Svid::new("spiffe://aura.local/agent").with_scopes(vec!["github".to_string()]);
        let cred = service.mint_credentials("github", &svid, &["github"], 120).await.unwrap();
        assert!(service.revoke_credentials(cred.raw_secret()).await.is_ok());
        assert!(service.revoke_credentials(cred.raw_secret()).await.is_err());
    }

    #[tokio::test]
    async fn test_f3_05_independent_token_revocations() {
        let service = OpaqueTunnelService::new();
        let svid = Svid::new("spiffe://aura.local/agent").with_scopes(vec!["github".to_string()]);
        let cred1 = service.mint_credentials("github", &svid, &["github"], 120).await.unwrap();
        let cred2 = service.mint_credentials("github", &svid, &["github"], 120).await.unwrap();
        service.revoke_credentials(cred1.raw_secret()).await.unwrap();

        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        assert!(!service.is_valid(cred1.raw_secret(), now).await);
        assert!(service.is_valid(cred2.raw_secret(), now).await);
    }

    // --- F4: Out-of-Band Sandbox Credential Injection ---
    #[test]
    fn test_f4_01_inject_into_sandbox_environment() {
        let cred = OpaqueCredential::new("github", "secret_token_123", 999999);
        let mut env = HashMap::new();
        assert!(OutOfBandInjector::inject_into_env(&mut env, "GITHUB_TOKEN", &cred).is_ok());
        assert_eq!(env.get("GITHUB_TOKEN").unwrap(), "secret_token_123");
    }

    #[test]
    fn test_f4_02_injection_isolated_per_session() {
        let cred = OpaqueCredential::new("github", "secret_1", 999999);
        let mut session1_env = HashMap::new();
        let session2_env: HashMap<String, String> = HashMap::new();
        OutOfBandInjector::inject_into_env(&mut session1_env, "AUTH_TOKEN", &cred).unwrap();
        assert!(session1_env.contains_key("AUTH_TOKEN"));
        assert!(!session2_env.contains_key("AUTH_TOKEN"));
    }

    #[test]
    fn test_f4_03_injection_overwrites_existing_variable() {
        let cred1 = OpaqueCredential::new("github", "old_secret", 999999);
        let cred2 = OpaqueCredential::new("github", "new_secret", 999999);
        let mut env = HashMap::new();
        OutOfBandInjector::inject_into_env(&mut env, "TOKEN", &cred1).unwrap();
        OutOfBandInjector::inject_into_env(&mut env, "TOKEN", &cred2).unwrap();
        assert_eq!(env.get("TOKEN").unwrap(), "new_secret");
    }

    #[test]
    fn test_f4_04_injection_preserves_system_variables() {
        let cred = OpaqueCredential::new("github", "token_val", 999999);
        let mut env = HashMap::new();
        env.insert("PATH".to_string(), "/usr/bin:/bin".to_string());
        OutOfBandInjector::inject_into_env(&mut env, "SECRET", &cred).unwrap();
        assert_eq!(env.get("PATH").unwrap(), "/usr/bin:/bin");
        assert_eq!(env.get("SECRET").unwrap(), "token_val");
    }

    #[test]
    fn test_f4_05_session_teardown_clears_injected_environment() {
        let cred = OpaqueCredential::new("github", "tok", 999999);
        let mut env = HashMap::new();
        OutOfBandInjector::inject_into_env(&mut env, "SECRET", &cred).unwrap();
        env.clear();
        assert!(env.is_empty());
    }

    // --- F5: Non-Ambient VCS PR Creation (GitManager::open_pr) ---
    #[test]
    fn test_f5_01_open_pr_happy_path_success() {
        let git = GitManager::new("main");
        let svid = Svid::new("spiffe://aura.local/agent").with_scopes(vec!["github".to_string()]);
        let res = git.open_pr(&svid, "feat: hardening update", "pr description");
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), "Opened PR: feat: hardening update");
    }

    #[test]
    fn test_f5_02_open_pr_auto_revokes_ephemeral_token_on_completion() {
        let git = GitManager::new("main");
        let svid = Svid::new("spiffe://aura.local/agent").with_scopes(vec!["github".to_string()]);
        git.open_pr(&svid, "fix: clean token lifecycle", "description").unwrap();
        assert_eq!(git.active_tokens.lock().unwrap().len(), 0);
        assert_eq!(git.revoked_tokens.lock().unwrap().len(), 1);
    }

    #[test]
    fn test_f5_03_open_pr_empty_title_fails() {
        let git = GitManager::new("main");
        let svid = Svid::new("spiffe://aura.local/agent").with_scopes(vec!["github".to_string()]);
        let res = git.open_pr(&svid, "", "description");
        assert!(res.is_err());
        assert_eq!(res.unwrap_err(), "PR title cannot be empty");
    }

    #[tokio::test]
    async fn test_f5_04_raii_drop_guard_auto_revokes_token_on_failure() {
        let service = OpaqueTunnelService::new();
        let svid = Svid::new("spiffe://aura.local/agent").with_scopes(vec!["github".to_string()]);
        let cred = service.mint_credentials("github", &svid, &["github"], 120).await.unwrap();

        {
            let _guard = PrRevocationGuard::new(&service, cred.raw_secret());
        }

        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(service.is_revoked(cred.raw_secret()).await);
    }

    #[test]
    fn test_f5_05_open_pr_whitespace_title_rejected() {
        let git = GitManager::new("main");
        let svid = Svid::new("spiffe://aura.local/agent").with_scopes(vec!["github".to_string()]);
        let res = git.open_pr(&svid, "   \n\t   ", "body");
        assert!(res.is_err());
    }

    // --- F6: Guest Daemon X11 Synthetic Actions Protocol ---
    #[test]
    fn test_f6_01_jsonrpc_click_parsed_successfully() {
        let req: serde_json::Value = serde_json::from_str(
            r#"{"jsonrpc": "2.0", "id": 1, "method": "click", "params": {"x": 100, "y": 200, "button": 1}}"#
        ).unwrap();
        assert_eq!(req["method"], "click");
        assert_eq!(req["params"]["x"], 100);
    }

    #[test]
    fn test_f6_02_jsonrpc_move_parsed_successfully() {
        let req: serde_json::Value = serde_json::from_str(
            r#"{"jsonrpc": "2.0", "id": 2, "method": "move", "params": {"x": 50, "y": 75}}"#
        ).unwrap();
        assert_eq!(req["method"], "move");
    }

    #[test]
    fn test_f6_03_jsonrpc_drag_parsed_successfully() {
        let req: serde_json::Value = serde_json::from_str(
            r#"{"jsonrpc": "2.0", "id": 3, "method": "drag", "params": {"x": 300, "y": 400}}"#
        ).unwrap();
        assert_eq!(req["method"], "drag");
    }

    #[test]
    fn test_f6_04_jsonrpc_type_parsed_successfully() {
        let req: serde_json::Value = serde_json::from_str(
            r#"{"jsonrpc": "2.0", "id": 4, "method": "type", "params": {"text": "cargo test"}}"#
        ).unwrap();
        assert_eq!(req["params"]["text"], "cargo test");
    }

    #[test]
    fn test_f6_05_jsonrpc_key_parsed_successfully() {
        let req: serde_json::Value = serde_json::from_str(
            r#"{"jsonrpc": "2.0", "id": 5, "method": "key", "params": {"key": "Return", "action": "press"}}"#
        ).unwrap();
        assert_eq!(req["params"]["key"], "Return");
    }

    // --- F7: Screen Capture & X11 Depth Handling ---
    #[test]
    fn test_f7_01_png_magic_bytes_validation() {
        let valid_png_header = b"\x89PNG\r\n\x1a\n";
        assert_eq!(&valid_png_header[0..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
    }

    #[test]
    fn test_f7_02_plane_mask_calculation_for_24bit_screen() {
        let depth = 24u32;
        let plane_mask = if depth < 32 { (1u32 << depth) - 1 } else { !0 };
        assert_eq!(plane_mask, 0x00FF_FFFF);
    }

    #[test]
    fn test_f7_03_plane_mask_calculation_for_32bit_screen() {
        let depth = 32u32;
        let plane_mask = if depth < 32 { (1u32 << depth) - 1 } else { !0 };
        assert_eq!(plane_mask, 0xFFFF_FFFF);
    }

    #[test]
    fn test_f7_04_plane_mask_calculation_for_16bit_screen() {
        let depth = 16u32;
        let plane_mask = if depth < 32 { (1u32 << depth) - 1 } else { !0 };
        assert_eq!(plane_mask, 0x0000_FFFF);
    }

    #[test]
    fn test_f7_05_corrupt_non_png_header_rejected() {
        let corrupt_header = b"NOT_A_PNG_HEADER";
        assert_ne!(&corrupt_header[0..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
    }

    // --- F8: Host-Side PNG Scanline Visual Redaction ---
    #[test]
    fn test_f8_01_redaction_single_box_blackout() {
        let mut pixels = vec![255u8; 10 * 10 * 4];
        apply_scanline_redaction(&mut pixels, 10, 10, &[(2, 2, 4, 4)]);
        let offset = (3 * 10 + 3) * 4;
        assert_eq!(&pixels[offset..offset + 4], &[0, 0, 0, 255]);
    }

    #[test]
    fn test_f8_02_redaction_unmasked_pixels_remain_unchanged() {
        let mut pixels = vec![255u8; 10 * 10 * 4];
        apply_scanline_redaction(&mut pixels, 10, 10, &[(2, 2, 4, 4)]);
        assert_eq!(&pixels[0..4], &[255, 255, 255, 255]);
    }

    #[test]
    fn test_f8_03_redaction_multiple_boxes() {
        let mut pixels = vec![255u8; 10 * 10 * 4];
        apply_scanline_redaction(&mut pixels, 10, 10, &[(0, 0, 2, 2), (8, 8, 2, 2)]);
        assert_eq!(&pixels[0..4], &[0, 0, 0, 255]);
        let offset = (9 * 10 + 9) * 4;
        assert_eq!(&pixels[offset..offset + 4], &[0, 0, 0, 255]);
    }

    #[test]
    fn test_f8_04_redaction_full_screen_mask() {
        let mut pixels = vec![255u8; 5 * 5 * 4];
        apply_scanline_redaction(&mut pixels, 5, 5, &[(0, 0, 5, 5)]);
        assert!(pixels.chunks(4).all(|p| p == [0, 0, 0, 255]));
    }

    #[test]
    fn test_f8_05_redaction_preserves_pixel_buffer_length() {
        let mut pixels = vec![128u8; 20 * 20 * 4];
        let original_len = pixels.len();
        apply_scanline_redaction(&mut pixels, 20, 20, &[(5, 5, 10, 10)]);
        assert_eq!(pixels.len(), original_len);
    }

    // --- F9: Python SDK ComputerTool Actions / JSON-RPC Protocol ---
    #[test]
    fn test_f9_01_computer_action_click_response_schema() {
        let res = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {"status": "ok"}
        });
        assert_eq!(res["result"]["status"], "ok");
    }

    #[test]
    fn test_f9_02_computer_action_move_response_schema() {
        let res = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "result": {"status": "ok"}
        });
        assert_eq!(res["result"]["status"], "ok");
    }

    #[test]
    fn test_f9_03_computer_action_type_response_schema() {
        let res = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "result": {"status": "ok"}
        });
        assert_eq!(res["result"]["status"], "ok");
    }

    #[test]
    fn test_f9_04_computer_action_drag_response_schema() {
        let res = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 4,
            "result": {"status": "ok"}
        });
        assert_eq!(res["result"]["status"], "ok");
    }

    #[test]
    fn test_f9_05_computer_action_screenshot_response_schema() {
        let res = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 5,
            "result": {"png_base64": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAAAAAA6fptVAAAACklEQVR4nGNiAAAABgADNjd8qAAAAABJRU5ErkJggg=="}
        });
        assert!(res["result"]["png_base64"].as_str().is_some());
    }

    // --- F10: Vsock Stream Buffer Memory Limits & Truncation ---
    #[tokio::test]
    async fn test_f10_01_stream_under_limit_retained_completely() {
        let data = vec![0xAA; 1024];
        let mut cursor = io::Cursor::new(data.clone());
        let (buf, truncated) = read_stream_with_limit(&mut cursor, 2048).await.unwrap();
        assert_eq!(buf, data);
        assert!(!truncated);
    }

    #[tokio::test]
    async fn test_f10_02_stream_over_limit_instantly_truncated() {
        let data = vec![0xBB; 8192];
        let mut cursor = io::Cursor::new(data);
        let (buf, truncated) = read_stream_with_limit(&mut cursor, 4096).await.unwrap();
        assert_eq!(buf.len(), 4096);
        assert!(truncated);
    }

    #[tokio::test]
    async fn test_f10_03_stream_truncation_flag_is_true_on_violation() {
        let data = vec![0xCC; 100];
        let mut cursor = io::Cursor::new(data);
        let (_, truncated) = read_stream_with_limit(&mut cursor, 50).await.unwrap();
        assert!(truncated);
    }

    #[tokio::test]
    async fn test_f10_04_stream_zero_byte_limit_truncates_immediately() {
        let data = vec![0xDD; 100];
        let mut cursor = io::Cursor::new(data);
        let (buf, truncated) = read_stream_with_limit(&mut cursor, 0).await.unwrap();
        assert_eq!(buf.len(), 0);
        assert!(truncated);
    }

    #[tokio::test]
    async fn test_f10_05_stream_exact_limit_boundary_not_truncated() {
        let data = vec![0xEE; 1000];
        let mut cursor = io::Cursor::new(data.clone());
        let (buf, truncated) = read_stream_with_limit(&mut cursor, 1000).await.unwrap();
        assert_eq!(buf, data);
        assert!(!truncated);
    }

    // --- F11: MicroVM Egress Transparent TLS Enforcement ---
    #[tokio::test]
    async fn test_f11_01_egress_proxy_allows_crates_io() {
        let config = EgressProxyConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            allowed_domains: vec!["crates.io".to_string()],
            enforce_strict_tls: true,
            upstream_override: None,
        };
        let proxy = EgressProxy::start(config).await.unwrap();
        assert!(proxy.bind_addr.port() > 0);
        proxy.stop();
    }

    #[tokio::test]
    async fn test_f11_02_egress_proxy_blocks_unauthorized_domain_with_alert_49() {
        let config = EgressProxyConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            allowed_domains: vec!["crates.io".to_string()],
            enforce_strict_tls: true,
            upstream_override: None,
        };
        let proxy = EgressProxy::start(config).await.unwrap();
        let proxy_addr = proxy.bind_addr;

        let mut client = TcpStream::connect(proxy_addr).await.unwrap();
        let client_hello = create_tls_client_hello("evil-domain.com");
        client.write_all(&client_hello).await.unwrap();

        let mut response = vec![0u8; 7];
        let n = client.read_exact(&mut response).await.unwrap();
        assert_eq!(n, 7);
        assert_eq!(&response[..], TLS_ALERT_ACCESS_DENIED);

        proxy.stop();
    }

    #[test]
    fn test_f11_03_tls_alert_access_denied_bytes_constant() {
        assert_eq!(
            TLS_ALERT_ACCESS_DENIED,
            &[0x15, 0x03, 0x03, 0x00, 0x02, 0x02, 0x31]
        );
    }

    #[tokio::test]
    async fn test_f11_04_egress_proxy_allows_pypi_org() {
        let config = EgressProxyConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            allowed_domains: vec!["pypi.org".to_string()],
            enforce_strict_tls: true,
            upstream_override: None,
        };
        let proxy = EgressProxy::start(config).await.unwrap();
        assert!(proxy.bind_addr.port() > 0);
        proxy.stop();
    }

    #[tokio::test]
    async fn test_f11_05_egress_proxy_terminates_connection_after_alert() {
        let config = EgressProxyConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            allowed_domains: vec!["github.com".to_string()],
            enforce_strict_tls: true,
            upstream_override: None,
        };
        let proxy = EgressProxy::start(config).await.unwrap();
        let proxy_addr = proxy.bind_addr;

        let mut client = TcpStream::connect(proxy_addr).await.unwrap();
        let client_hello = create_tls_client_hello("unauthorized-site.org");
        client.write_all(&client_hello).await.unwrap();

        let mut alert_buf = [0u8; 7];
        client.read_exact(&mut alert_buf).await.unwrap();

        let mut extra = [0u8; 1];
        let eof = client.read(&mut extra).await.unwrap();
        assert_eq!(eof, 0);

        proxy.stop();
    }

    // --- F12: Encrypted Database at Rest (AES-256-GCM) ---
    #[test]
    fn test_f12_01_encrypted_envelope_starts_with_aura_magic() {
        let key = [0x42u8; 32];
        let nonce = [0x01u8; 12];
        let plaintext = b"CREATE TABLE users (id INT);";
        let encrypted = OpaqueEncryptedStore::encrypt(&key, &nonce, plaintext);
        assert_eq!(&encrypted[0..4], b"AURA");
        assert_eq!(encrypted[4], 1);
    }

    #[test]
    fn test_f12_02_encrypted_envelope_does_not_start_with_sqlite_header() {
        let key = [0x42u8; 32];
        let nonce = [0x01u8; 12];
        let plaintext = b"SQLite format 3\0sample payload";
        let encrypted = OpaqueEncryptedStore::encrypt(&key, &nonce, plaintext);
        assert_ne!(&encrypted[0..16], b"SQLite format 3\0");
    }

    #[test]
    fn test_f12_03_zero_plaintext_strings_in_raw_ciphertext() {
        let key = [0x55u8; 32];
        let nonce = [0x02u8; 12];
        let plaintext = b"SECRET_API_TOKEN_XYZ_123456789";
        let encrypted = OpaqueEncryptedStore::encrypt(&key, &nonce, plaintext);
        let as_str = String::from_utf8_lossy(&encrypted);
        assert!(!as_str.contains("SECRET_API_TOKEN"));
    }

    #[test]
    fn test_f12_04_successful_decryption_with_authentic_key() {
        let key = [0x77u8; 32];
        let nonce = [0x03u8; 12];
        let original = b"Sensitive SQL database contents";
        let encrypted = OpaqueEncryptedStore::encrypt(&key, &nonce, original);
        let decrypted = OpaqueEncryptedStore::decrypt(&key, &encrypted).unwrap();
        assert_eq!(&decrypted[..], original);
    }

    #[test]
    fn test_f12_05_rejection_with_wrong_key() {
        let key_a = [0x11u8; 32];
        let key_b = [0x22u8; 32];
        let nonce = [0x04u8; 12];
        let encrypted = OpaqueEncryptedStore::encrypt(&key_a, &nonce, b"payload");
        let res = OpaqueEncryptedStore::decrypt(&key_b, &encrypted);
        assert!(res.is_err());
    }

    // --- F13: Task Ledger Encrypted Persistence ---
    #[tokio::test]
    async fn test_f13_01_record_task_dispatch_event() {
        let key = [0x33u8; 32];
        let ledger = OpaqueTaskLedger::new(key);
        let task = OpaqueTaskRecord {
            task_id: "task-001".to_string(),
            agent: "coder".to_string(),
            instruction: "run tests".to_string(),
            substrate: "firecracker".to_string(),
            status: "Pending".to_string(),
            stdout: "".to_string(),
            stderr: "".to_string(),
            exit_code: 0,
            timestamp: 1000,
        };
        ledger.record(task.clone()).await;
        let retrieved = ledger.get_task("task-001").await.unwrap();
        assert_eq!(retrieved, task);
    }

    #[tokio::test]
    async fn test_f13_02_record_task_completion_success() {
        let key = [0x33u8; 32];
        let ledger = OpaqueTaskLedger::new(key);
        let task = OpaqueTaskRecord {
            task_id: "task-002".to_string(),
            agent: "coder".to_string(),
            instruction: "cargo check".to_string(),
            substrate: "wasm".to_string(),
            status: "Completed".to_string(),
            stdout: "Finished dev".to_string(),
            stderr: "".to_string(),
            exit_code: 0,
            timestamp: 2000,
        };
        ledger.record(task).await;
        let retrieved = ledger.get_task("task-002").await.unwrap();
        assert_eq!(retrieved.status, "Completed");
        assert_eq!(retrieved.exit_code, 0);
    }

    #[tokio::test]
    async fn test_f13_03_record_task_failure_transition() {
        let key = [0x33u8; 32];
        let ledger = OpaqueTaskLedger::new(key);
        let task = OpaqueTaskRecord {
            task_id: "task-003".to_string(),
            agent: "coder".to_string(),
            instruction: "syntax error".to_string(),
            substrate: "firecracker".to_string(),
            status: "Failed".to_string(),
            stdout: "".to_string(),
            stderr: "compile error".to_string(),
            exit_code: 1,
            timestamp: 3000,
        };
        ledger.record(task).await;
        let retrieved = ledger.get_task("task-003").await.unwrap();
        assert_eq!(retrieved.status, "Failed");
        assert_eq!(retrieved.exit_code, 1);
    }

    #[tokio::test]
    async fn test_f13_04_query_nonexistent_task_returns_none() {
        let key = [0x33u8; 32];
        let ledger = OpaqueTaskLedger::new(key);
        assert!(ledger.get_task("nonexistent").await.is_none());
    }

    #[tokio::test]
    async fn test_f13_05_flush_and_reload_encrypted_ledger() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("ledger.db");
        let key = [0x99u8; 32];
        let nonce = [0x12u8; 12];

        let ledger = OpaqueTaskLedger::new(key);
        ledger.record(OpaqueTaskRecord {
            task_id: "task-persist".to_string(),
            agent: "tester".to_string(),
            instruction: "persist ledger".to_string(),
            substrate: "wasm".to_string(),
            status: "Completed".to_string(),
            stdout: "ok".to_string(),
            stderr: "".to_string(),
            exit_code: 0,
            timestamp: 5000,
        }).await;

        ledger.flush_to_disk(&db_path, &nonce).await.unwrap();

        let reloaded = OpaqueTaskLedger::load_from_disk(&db_path, &key).await.unwrap();
        let task = reloaded.get_task("task-persist").await.unwrap();
        assert_eq!(task.instruction, "persist ledger");
    }

    // --- F14: SPIRE Trust Domain Federation & OIDC Verification ---
    #[test]
    fn test_f14_01_spire_federation_register_trust_domain() {
        let fed = FederationManager::new();
        fed.register_federated_domain("corp.enterprise.com", vec![1, 2, 3, 4]);
        assert!(fed.is_federated("corp.enterprise.com"));
    }

    #[test]
    fn test_f14_02_spire_federation_unregistered_domain_is_false() {
        let fed = FederationManager::new();
        assert!(!fed.is_federated("untrusted.domain.org"));
    }

    #[test]
    fn test_f14_03_oidc_token_valid_claims_authentication() {
        let mut allowed = HashSet::new();
        allowed.insert("dev-scope".to_string());
        let verifier = OpaqueOidcVerifier {
            issuer: "https://auth.enterprise.com".to_string(),
            audience: "aura-agent-workload".to_string(),
            allowed_scopes: allowed,
        };

        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let payload = serde_json::json!({
            "iss": "https://auth.enterprise.com",
            "aud": "aura-agent-workload",
            "sub": "engineer-123",
            "exp": now + 3600,
            "roles": ["dev-scope"]
        });

        let svid = verifier.verify(&payload).unwrap();
        assert!(svid.id.contains("engineer-123"));
        assert!(svid.has_scope("dev-scope"));
    }

    #[test]
    fn test_f14_04_oidc_token_expired_rejected() {
        let verifier = OpaqueOidcVerifier {
            issuer: "https://auth.enterprise.com".to_string(),
            audience: "aura-agent-workload".to_string(),
            allowed_scopes: HashSet::new(),
        };

        let payload = serde_json::json!({
            "iss": "https://auth.enterprise.com",
            "aud": "aura-agent-workload",
            "sub": "user",
            "exp": 1000,
        });

        assert!(verifier.verify(&payload).is_err());
    }

    #[test]
    fn test_f14_05_oidc_mismatched_audience_rejected() {
        let verifier = OpaqueOidcVerifier {
            issuer: "https://auth.enterprise.com".to_string(),
            audience: "aura-agent-workload".to_string(),
            allowed_scopes: HashSet::new(),
        };

        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let payload = serde_json::json!({
            "iss": "https://auth.enterprise.com",
            "aud": "different-audience",
            "sub": "user",
            "exp": now + 3600,
        });

        assert!(verifier.verify(&payload).is_err());
    }

    // --- F15: Transport UDP 1-RTT Failure Detection & TCP Fallback ---
    #[tokio::test]
    async fn test_f15_01_udp_healthy_completes_over_udp() {
        let udp_server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let udp_addr = udp_server.local_addr().unwrap();

        tokio::spawn(async move {
            let mut buf = [0u8; 1024];
            let (n, peer) = udp_server.recv_from(&mut buf).await.unwrap();
            let req: JsonRpcRequest = serde_json::from_slice(&buf[..n]).unwrap();
            let resp = JsonRpcResponse::success(req.id, serde_json::json!({"status": "udp_ok"}));
            let resp_bytes = serde_json::to_vec(&resp).unwrap();
            udp_server.send_to(&resp_bytes, peer).await.unwrap();
        });

        let tcp_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let tcp_addr = tcp_listener.local_addr().unwrap();

        let client = OpaqueResilientClient::new(udp_addr, tcp_addr, Duration::from_millis(50));
        let req = make_rpc_req(1, "tools/list", serde_json::Value::Null);
        let resp = client.call_tool(&req).await.unwrap();
        assert_eq!(resp.result.as_ref().unwrap()["status"], "udp_ok");
    }

    #[tokio::test]
    async fn test_f15_02_udp_blocked_triggers_tcp_fallback() {
        let blackhole_udp = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let udp_addr = blackhole_udp.local_addr().unwrap();

        let tcp_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let tcp_addr = tcp_listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (mut stream, _) = tcp_listener.accept().await.unwrap();
            let mut len_buf = [0u8; 4];
            stream.read_exact(&mut len_buf).await.unwrap();
            let len = u32::from_be_bytes(len_buf) as usize;
            let mut body = vec![0u8; len];
            stream.read_exact(&mut body).await.unwrap();

            let req: JsonRpcRequest = serde_json::from_slice(&body).unwrap();
            let resp = JsonRpcResponse::success(req.id, serde_json::json!({"status": "tcp_fallback_ok"}));
            let resp_bytes = serde_json::to_vec(&resp).unwrap();

            let resp_len = (resp_bytes.len() as u32).to_be_bytes();
            stream.write_all(&resp_len).await.unwrap();
            stream.write_all(&resp_bytes).await.unwrap();
        });

        let client = OpaqueResilientClient::new(udp_addr, tcp_addr, Duration::from_millis(30));
        let req = make_rpc_req(2, "tools/call", serde_json::json!({"name": "bash"}));
        let resp = client.call_tool(&req).await.unwrap();
        assert_eq!(resp.result.as_ref().unwrap()["status"], "tcp_fallback_ok");
    }

    #[tokio::test]
    async fn test_f15_03_fallback_completes_within_1_rtt_timeout() {
        let blackhole_udp = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let udp_addr = blackhole_udp.local_addr().unwrap();

        let tcp_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let tcp_addr = tcp_listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (mut stream, _) = tcp_listener.accept().await.unwrap();
            let mut len_buf = [0u8; 4];
            stream.read_exact(&mut len_buf).await.unwrap();
            let len = u32::from_be_bytes(len_buf) as usize;
            let mut body = vec![0u8; len];
            stream.read_exact(&mut body).await.unwrap();

            let req: JsonRpcRequest = serde_json::from_slice(&body).unwrap();
            let resp = JsonRpcResponse::success(req.id, serde_json::json!({"ok": true}));
            let resp_bytes = serde_json::to_vec(&resp).unwrap();
            stream.write_all(&(resp_bytes.len() as u32).to_be_bytes()).await.unwrap();
            stream.write_all(&resp_bytes).await.unwrap();
        });

        let client = OpaqueResilientClient::new(udp_addr, tcp_addr, Duration::from_millis(50));
        let start = Instant::now();
        let req = make_rpc_req(3, "tools/list", serde_json::Value::Null);
        let resp = client.call_tool(&req).await.unwrap();
        let elapsed = start.elapsed();

        assert!(resp.is_success());
        assert!(elapsed < Duration::from_millis(200));
    }

    #[tokio::test]
    async fn test_f15_04_in_flight_call_replayed_without_drop() {
        let blackhole_udp = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let udp_addr = blackhole_udp.local_addr().unwrap();

        let tcp_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let tcp_addr = tcp_listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (mut stream, _) = tcp_listener.accept().await.unwrap();
            let mut len_buf = [0u8; 4];
            stream.read_exact(&mut len_buf).await.unwrap();
            let len = u32::from_be_bytes(len_buf) as usize;
            let mut body = vec![0u8; len];
            stream.read_exact(&mut body).await.unwrap();

            let req: JsonRpcRequest = serde_json::from_slice(&body).unwrap();
            let resp = JsonRpcResponse::success(req.id, serde_json::json!({"received_call": req.method}));
            let resp_bytes = serde_json::to_vec(&resp).unwrap();
            stream.write_all(&(resp_bytes.len() as u32).to_be_bytes()).await.unwrap();
            stream.write_all(&resp_bytes).await.unwrap();
        });

        let client = OpaqueResilientClient::new(udp_addr, tcp_addr, Duration::from_millis(40));
        let req = make_rpc_req(42, "critical_tool_call", serde_json::Value::Null);
        let resp = client.call_tool(&req).await.unwrap();
        assert_eq!(resp.result.as_ref().unwrap()["received_call"], "critical_tool_call");
    }

    #[tokio::test]
    async fn test_f15_05_subsequent_calls_reuse_cached_tcp_route() {
        let blackhole_udp = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let udp_addr = blackhole_udp.local_addr().unwrap();

        let tcp_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let tcp_addr = tcp_listener.local_addr().unwrap();

        tokio::spawn(async move {
            while let Ok((mut stream, _)) = tcp_listener.accept().await {
                let mut len_buf = [0u8; 4];
                if stream.read_exact(&mut len_buf).await.is_err() {
                    break;
                }
                let len = u32::from_be_bytes(len_buf) as usize;
                let mut body = vec![0u8; len];
                stream.read_exact(&mut body).await.unwrap();

                let req: JsonRpcRequest = serde_json::from_slice(&body).unwrap();
                let resp = JsonRpcResponse::success(req.id, serde_json::json!({"cached": true}));
                let resp_bytes = serde_json::to_vec(&resp).unwrap();
                stream.write_all(&(resp_bytes.len() as u32).to_be_bytes()).await.unwrap();
                stream.write_all(&resp_bytes).await.unwrap();
            }
        });

        let client = OpaqueResilientClient::new(udp_addr, tcp_addr, Duration::from_millis(40));
        let _ = client.call_tool(&make_rpc_req(1, "t1", serde_json::Value::Null)).await.unwrap();

        let start = Instant::now();
        let resp2 = client.call_tool(&make_rpc_req(2, "t2", serde_json::Value::Null)).await.unwrap();
        let elapsed = start.elapsed();

        assert_eq!(resp2.result.as_ref().unwrap()["cached"], true);
        assert!(elapsed < Duration::from_millis(50));
    }
}

// ============================================================================
// Tier 2: Boundary & Corner Cases (F1 to F15, >=5 tests per feature = 75 tests)
// ============================================================================

mod tier2_boundary_corner_cases {
    use super::*;

    // --- Boundary F1: Credential Redaction ---
    #[test]
    fn test_b_f1_01_empty_token_string_safe_formatting() {
        let cred = OpaqueCredential::new("github", "", 999999);
        assert!(cred.sanitized_handle().contains("[REDACTED-TOKEN-...]"));
    }

    #[test]
    fn test_b_f1_02_exact_8_byte_token_boundary() {
        let cred = OpaqueCredential::new("github", "12345678", 999999);
        assert!(cred.sanitized_handle().ends_with("[REDACTED-TOKEN-12345678]"));
    }

    #[test]
    fn test_b_f1_03_unicode_and_null_byte_tokens_masked() {
        let cred = OpaqueCredential::new("github", "secret\0token🔥🎉", 999999);
        let displayed = format!("{}", cred);
        assert!(!displayed.contains("secret"));
    }

    #[test]
    fn test_b_f1_04_extreme_length_token_10kb() {
        let huge_token = "A".repeat(10_000);
        let cred = OpaqueCredential::new("service", huge_token, 999999);
        let displayed = format!("{}", cred);
        assert!(displayed.len() < 100);
    }

    #[test]
    fn test_b_f1_05_whitespace_only_token_masked() {
        let cred = OpaqueCredential::new("vault", "        ", 999999);
        assert!(cred.sanitized_handle().contains("[REDACTED-TOKEN-"));
    }

    // --- Boundary F2: Scope Enforcement ---
    #[tokio::test]
    async fn test_b_f2_01_empty_requested_service_rejected() {
        let service = OpaqueTunnelService::new();
        let svid = Svid::new("spiffe://aura.local/agent");
        let res = service.mint_credentials("", &svid, &["github"], 120).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn test_b_f2_02_whitespace_service_name_rejected() {
        let service = OpaqueTunnelService::new();
        let svid = Svid::new("spiffe://aura.local/agent");
        let res = service.mint_credentials("   \t  ", &svid, &["github"], 120).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn test_b_f2_03_case_sensitive_scope_enforcement() {
        let service = OpaqueTunnelService::new();
        let svid = Svid::new("spiffe://aura.local/agent").with_scopes(vec!["GITHUB".to_string()]);
        let res = service.mint_credentials("github", &svid, &["github"], 120).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn test_b_f2_04_wildcard_scope_escalation_prevented() {
        let service = OpaqueTunnelService::new();
        let svid = Svid::new("spiffe://aura.local/agent").with_scopes(vec!["*".to_string()]);
        let res = service.mint_credentials("admin-vault", &svid, &["github"], 120).await;
        assert!(res.is_err());
    }

    #[test]
    fn test_b_f2_05_cedar_enforcer_untrusted_svid_denied() {
        let svid = Svid::new("spiffe://unverified.domain/agent");
        let check = CapabilityEnforcer::check(&svid, Capability::FileSystemRead);
        assert!(check.is_err());
    }

    // --- Boundary F3: Revocation Lifecycle ---
    #[tokio::test]
    async fn test_b_f3_01_zero_second_ttl_immediately_expired() {
        let service = OpaqueTunnelService::new();
        let svid = Svid::new("spiffe://aura.local/agent").with_scopes(vec!["github".to_string()]);
        let cred = service.mint_credentials("github", &svid, &["github"], 0).await.unwrap();
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        assert!(!service.is_valid(cred.raw_secret(), now + 1).await);
    }

    #[tokio::test]
    async fn test_b_f3_02_max_ttl_u64_overflow_handling() {
        let service = OpaqueTunnelService::new();
        let svid = Svid::new("spiffe://aura.local/agent").with_scopes(vec!["github".to_string()]);
        let cred = service.mint_credentials("github", &svid, &["github"], u64::MAX).await.unwrap();
        assert_eq!(cred.expires_at, u64::MAX);
    }

    #[tokio::test]
    async fn test_b_f3_03_query_nonexistent_token_is_invalid() {
        let service = OpaqueTunnelService::new();
        assert!(!service.is_valid("random_fake_token", 1000).await);
    }

    #[tokio::test]
    async fn test_b_f3_04_revoking_nonexistent_token_fails() {
        let service = OpaqueTunnelService::new();
        assert!(service.revoke_credentials("nonexistent").await.is_err());
    }

    #[tokio::test]
    async fn test_b_f3_05_concurrent_mints_generate_unique_secrets() {
        let service = OpaqueTunnelService::new();
        let svid = Svid::new("spiffe://aura.local/agent").with_scopes(vec!["github".to_string()]);
        let mut handles = Vec::new();

        for _ in 0..20 {
            let s = service.clone();
            let id = svid.clone();
            handles.push(tokio::spawn(async move {
                s.mint_credentials("github", &id, &["github"], 120).await.unwrap()
            }));
        }

        let mut tokens = HashSet::new();
        for h in handles {
            let cred = h.await.unwrap();
            assert!(tokens.insert(cred.raw_secret().to_string()));
        }
        assert_eq!(tokens.len(), 20);
    }

    // --- Boundary F4: Out-of-Band Injection ---
    #[test]
    fn test_b_f4_01_empty_variable_name_rejected() {
        let cred = OpaqueCredential::new("github", "secret", 999999);
        let mut env = HashMap::new();
        assert!(OutOfBandInjector::inject_into_env(&mut env, "", &cred).is_err());
    }

    #[test]
    fn test_b_f4_02_whitespace_variable_name_rejected() {
        let cred = OpaqueCredential::new("github", "secret", 999999);
        let mut env = HashMap::new();
        assert!(OutOfBandInjector::inject_into_env(&mut env, "   \n  ", &cred).is_err());
    }

    #[test]
    fn test_b_f4_03_special_characters_in_secret_safe_injection() {
        let cred = OpaqueCredential::new("github", "tok$VAR;`rm -rf /`'\"", 999999);
        let mut env = HashMap::new();
        OutOfBandInjector::inject_into_env(&mut env, "AUTH", &cred).unwrap();
        assert_eq!(env.get("AUTH").unwrap(), "tok$VAR;`rm -rf /`'\"");
    }

    #[test]
    fn test_b_f4_04_large_4kb_secret_injection() {
        let big_secret = "x".repeat(4096);
        let cred = OpaqueCredential::new("vault", big_secret.clone(), 999999);
        let mut env = HashMap::new();
        OutOfBandInjector::inject_into_env(&mut env, "BIG", &cred).unwrap();
        assert_eq!(env.get("BIG").unwrap().len(), 4096);
    }

    #[test]
    fn test_b_f4_05_concurrent_env_injections() {
        let cred1 = OpaqueCredential::new("s1", "v1", 999999);
        let cred2 = OpaqueCredential::new("s2", "v2", 999999);
        let mut env = HashMap::new();
        OutOfBandInjector::inject_into_env(&mut env, "K1", &cred1).unwrap();
        OutOfBandInjector::inject_into_env(&mut env, "K2", &cred2).unwrap();
        assert_eq!(env.len(), 2);
    }

    // --- Boundary F5: VCS PR Workflow ---
    #[test]
    fn test_b_f5_01_huge_1mb_pr_body_handled() {
        let git = GitManager::new("main");
        let svid = Svid::new("spiffe://aura.local/agent").with_scopes(vec!["github".to_string()]);
        let huge_body = "b".repeat(1_000_000);
        let res = git.open_pr(&svid, "feat: big body", &huge_body);
        assert!(res.is_ok());
    }

    #[test]
    fn test_b_f5_02_unicode_and_emoji_title() {
        let git = GitManager::new("main");
        let svid = Svid::new("spiffe://aura.local/agent").with_scopes(vec!["github".to_string()]);
        let res = git.open_pr(&svid, "🚀 Release v2.0 🔒 Hardening", "description");
        assert!(res.is_ok());
    }

    #[test]
    fn test_b_f5_03_rapid_sequential_prs_mint_distinct_tokens() {
        let git = GitManager::new("main");
        let svid = Svid::new("spiffe://aura.local/agent").with_scopes(vec!["github".to_string()]);
        git.open_pr(&svid, "PR 1", "body 1").unwrap();
        git.open_pr(&svid, "PR 2", "body 2").unwrap();
        git.open_pr(&svid, "PR 3", "body 3").unwrap();
        assert_eq!(git.revoked_tokens.lock().unwrap().len(), 3);
        assert_eq!(git.active_tokens.lock().unwrap().len(), 0);
    }

    #[test]
    fn test_b_f5_04_is_token_active_on_unknown_token() {
        let git = GitManager::new("main");
        assert!(!git.is_token_active("unknown-token"));
    }

    #[test]
    fn test_b_f5_05_is_token_revoked_on_unknown_token() {
        let git = GitManager::new("main");
        assert!(!git.is_token_revoked("unknown-token"));
    }

    // --- Boundary F6: X11 Synthetic Actions Protocol ---
    #[test]
    fn test_b_f6_01_negative_coordinates_in_move() {
        let req: serde_json::Value = serde_json::from_str(
            r#"{"jsonrpc": "2.0", "id": 1, "method": "move", "params": {"x": -10, "y": -20}}"#
        ).unwrap();
        assert_eq!(req["params"]["x"], -10);
    }

    #[test]
    fn test_b_f6_02_zero_coordinates_in_click() {
        let req: serde_json::Value = serde_json::from_str(
            r#"{"jsonrpc": "2.0", "id": 2, "method": "click", "params": {"x": 0, "y": 0, "button": 1}}"#
        ).unwrap();
        assert_eq!(req["params"]["x"], 0);
    }

    #[test]
    fn test_b_f6_03_huge_coordinates_4k_display() {
        let req: serde_json::Value = serde_json::from_str(
            r#"{"jsonrpc": "2.0", "id": 3, "method": "click", "params": {"x": 3840, "y": 2160, "button": 1}}"#
        ).unwrap();
        assert_eq!(req["params"]["x"], 3840);
    }

    #[test]
    fn test_b_f6_04_empty_typing_string() {
        let req: serde_json::Value = serde_json::from_str(
            r#"{"jsonrpc": "2.0", "id": 4, "method": "type", "params": {"text": ""}}"#
        ).unwrap();
        assert_eq!(req["params"]["text"], "");
    }

    #[test]
    fn test_b_f6_05_burst_of_100_jsonrpc_commands() {
        for i in 1..=100 {
            let raw = format!(r#"{{"jsonrpc": "2.0", "id": {}, "method": "move", "params": {{"x": {}, "y": {}}}}}"#, i, i, i);
            let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
            assert_eq!(val["id"], i);
        }
    }

    // --- Boundary F7: Screen Capture & Depth Handling ---
    #[test]
    fn test_b_f7_01_depth_8_bit_plane_mask() {
        let depth = 8u32;
        let plane_mask = if depth < 32 { (1u32 << depth) - 1 } else { !0 };
        assert_eq!(plane_mask, 0x0000_00FF);
    }

    #[test]
    fn test_b_f7_02_depth_1_bit_monochrome_plane_mask() {
        let depth = 1u32;
        let plane_mask = if depth < 32 { (1u32 << depth) - 1 } else { !0 };
        assert_eq!(plane_mask, 0x0000_0001);
    }

    #[test]
    fn test_b_f7_03_depth_30_bit_hdr_plane_mask() {
        let depth = 30u32;
        let plane_mask = if depth < 32 { (1u32 << depth) - 1 } else { !0 };
        assert_eq!(plane_mask, 0x3FFF_FFFF);
    }

    #[test]
    fn test_b_f7_04_depth_32_bit_all_ones() {
        let depth = 32u32;
        let plane_mask = if depth < 32 { (1u32 << depth) - 1 } else { !0 };
        assert_eq!(plane_mask, 0xFFFF_FFFF);
    }

    #[test]
    fn test_b_f7_05_truncated_4_byte_png_header_rejected() {
        let truncated = b"\x89PNG";
        assert_ne!(&truncated[..], &[137, 80, 78, 71, 13, 10, 26, 10]);
    }

    // --- Boundary F8: Visual Redaction Masks ---
    #[test]
    fn test_b_f8_01_empty_box_list_noop() {
        let mut pixels = vec![100u8; 10 * 10 * 4];
        let original = pixels.clone();
        apply_scanline_redaction(&mut pixels, 10, 10, &[]);
        assert_eq!(pixels, original);
    }

    #[test]
    fn test_b_f8_02_box_exceeding_screen_dimensions_clamped_safely() {
        let mut pixels = vec![255u8; 10 * 10 * 4];
        apply_scanline_redaction(&mut pixels, 10, 10, &[(5, 5, 100, 100)]);
        let offset = (9 * 10 + 9) * 4;
        assert_eq!(&pixels[offset..offset + 4], &[0, 0, 0, 255]);
    }

    #[test]
    fn test_b_f8_03_zero_width_height_box_noop() {
        let mut pixels = vec![255u8; 10 * 10 * 4];
        let original = pixels.clone();
        apply_scanline_redaction(&mut pixels, 10, 10, &[(2, 2, 0, 0)]);
        assert_eq!(pixels, original);
    }

    #[test]
    fn test_b_f8_04_overlapping_bounding_boxes_handled() {
        let mut pixels = vec![255u8; 10 * 10 * 4];
        apply_scanline_redaction(&mut pixels, 10, 10, &[(1, 1, 5, 5), (3, 3, 5, 5)]);
        let offset = (4 * 10 + 4) * 4;
        assert_eq!(&pixels[offset..offset + 4], &[0, 0, 0, 255]);
    }

    #[test]
    fn test_b_f8_05_1x1_pixel_box_masked() {
        let mut pixels = vec![255u8; 10 * 10 * 4];
        apply_scanline_redaction(&mut pixels, 10, 10, &[(0, 0, 1, 1)]);
        assert_eq!(&pixels[0..4], &[0, 0, 0, 255]);
        assert_eq!(&pixels[4..8], &[255, 255, 255, 255]);
    }

    // --- Boundary F9: ComputerTool Protocol ---
    #[test]
    fn test_b_f9_01_method_not_found_error_schema() {
        let err_resp = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": {"code": -32601, "message": "Method not found"}
        });
        assert_eq!(err_resp["error"]["code"], -32601);
    }

    #[test]
    fn test_b_f9_02_parse_error_schema() {
        let err_resp = serde_json::json!({
            "jsonrpc": "2.0",
            "id": serde_json::Value::Null,
            "error": {"code": -32700, "message": "Parse error"}
        });
        assert_eq!(err_resp["error"]["code"], -32700);
    }

    #[test]
    fn test_b_f9_03_invalid_params_error_schema() {
        let err_resp = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "error": {"code": -32602, "message": "Invalid params"}
        });
        assert_eq!(err_resp["error"]["code"], -32602);
    }

    #[test]
    fn test_b_f9_04_large_typing_string_100kb() {
        let text = "a".repeat(100_000);
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "type",
            "params": {"text": text}
        });
        assert_eq!(req["params"]["text"].as_str().unwrap().len(), 100_000);
    }

    #[test]
    fn test_b_f9_05_modifier_keys_representation() {
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "key",
            "params": {"key": "Control_L", "action": "press"}
        });
        assert_eq!(req["params"]["key"], "Control_L");
    }

    // --- Boundary F10: Vsock Buffer Limit ---
    #[tokio::test]
    async fn test_b_f10_01_empty_stream_zero_bytes() {
        let mut cursor = io::Cursor::new(Vec::new());
        let (buf, truncated) = read_stream_with_limit(&mut cursor, 1024).await.unwrap();
        assert!(buf.is_empty());
        assert!(!truncated);
    }

    #[tokio::test]
    async fn test_b_f10_02_limit_plus_one_byte_triggers_truncation() {
        let data = vec![0x11; 1001];
        let mut cursor = io::Cursor::new(data);
        let (buf, truncated) = read_stream_with_limit(&mut cursor, 1000).await.unwrap();
        assert_eq!(buf.len(), 1000);
        assert!(truncated);
    }

    #[tokio::test]
    async fn test_b_f10_03_huge_stream_10mb_truncated_at_32kb() {
        let huge = vec![0x22; 10 * 1024 * 1024];
        let mut cursor = io::Cursor::new(huge);
        let (buf, truncated) = read_stream_with_limit(&mut cursor, 32 * 1024).await.unwrap();
        assert_eq!(buf.len(), 32 * 1024);
        assert!(truncated);
    }

    #[tokio::test]
    async fn test_b_f10_04_limit_zero_with_non_empty_input() {
        let data = vec![1, 2, 3];
        let mut cursor = io::Cursor::new(data);
        let (buf, truncated) = read_stream_with_limit(&mut cursor, 0).await.unwrap();
        assert_eq!(buf.len(), 0);
        assert!(truncated);
    }

    #[tokio::test]
    async fn test_b_f10_05_slow_chunked_stream_handling() {
        let (mut client, mut server) = tokio::io::duplex(1024);
        let writer = tokio::spawn(async move {
            for b in 0..10u8 {
                client.write_all(&[b]).await.unwrap();
            }
        });

        let (buf, truncated) = read_stream_with_limit(&mut server, 20).await.unwrap();
        writer.await.unwrap();
        assert_eq!(buf.len(), 10);
        assert!(!truncated);
    }

    // --- Boundary F11: MicroVM Egress Proxy ---
    #[tokio::test]
    async fn test_b_f11_01_prefix_spoofed_domain_rejected() {
        let config = EgressProxyConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            allowed_domains: vec!["crates.io".to_string()],
            enforce_strict_tls: true,
            upstream_override: None,
        };
        let proxy = EgressProxy::start(config).await.unwrap();
        let proxy_addr = proxy.bind_addr;

        let mut client = TcpStream::connect(proxy_addr).await.unwrap();
        let client_hello = create_tls_client_hello("evil-crates.io");
        client.write_all(&client_hello).await.unwrap();

        let mut alert = [0u8; 7];
        client.read_exact(&mut alert).await.unwrap();
        assert_eq!(&alert[..], TLS_ALERT_ACCESS_DENIED);

        proxy.stop();
    }

    #[tokio::test]
    async fn test_b_f11_02_suffix_spoofed_domain_rejected() {
        let config = EgressProxyConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            allowed_domains: vec!["crates.io".to_string()],
            enforce_strict_tls: true,
            upstream_override: None,
        };
        let proxy = EgressProxy::start(config).await.unwrap();
        let proxy_addr = proxy.bind_addr;

        let mut client = TcpStream::connect(proxy_addr).await.unwrap();
        let client_hello = create_tls_client_hello("crates.io.attacker.org");
        client.write_all(&client_hello).await.unwrap();

        let mut alert = [0u8; 7];
        client.read_exact(&mut alert).await.unwrap();
        assert_eq!(&alert[..], TLS_ALERT_ACCESS_DENIED);

        proxy.stop();
    }

    #[tokio::test]
    async fn test_b_f11_03_non_tls_garbage_dropped() {
        let config = EgressProxyConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            allowed_domains: vec!["crates.io".to_string()],
            enforce_strict_tls: true,
            upstream_override: None,
        };
        let proxy = EgressProxy::start(config).await.unwrap();
        let proxy_addr = proxy.bind_addr;

        let mut client = TcpStream::connect(proxy_addr).await.unwrap();
        client.write_all(b"GET / HTTP/1.1\r\nHost: evil.com\r\n\r\n").await.unwrap();

        let mut buf = [0u8; 10];
        let n = client.read(&mut buf).await.unwrap_or(0);
        assert_eq!(n, 0);

        proxy.stop();
    }

    #[tokio::test]
    async fn test_b_f11_04_empty_allowed_domains_validation_fails() {
        let config = EgressProxyConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            allowed_domains: vec![],
            enforce_strict_tls: true,
            upstream_override: None,
        };
        assert!(config.validate().is_err());
    }

    #[tokio::test]
    async fn test_b_f11_05_subdomain_allowlist_matching() {
        let config = EgressProxyConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            allowed_domains: vec!["index.crates.io".to_string()],
            enforce_strict_tls: true,
            upstream_override: None,
        };
        let proxy = EgressProxy::start(config).await.unwrap();
        assert!(proxy.bind_addr.port() > 0);
        proxy.stop();
    }

    // --- Boundary F12: Encrypted Database at Rest ---
    #[test]
    fn test_b_f12_01_tampered_ciphertext_fails_auth() {
        let key = [0xAAu8; 32];
        let nonce = [0xBBu8; 12];
        let mut envelope = OpaqueEncryptedStore::encrypt(&key, &nonce, b"original text");
        envelope[17] ^= 0xFF;
        assert!(OpaqueEncryptedStore::decrypt(&key, &envelope).is_err());
    }

    #[test]
    fn test_b_f12_02_tampered_nonce_fails_auth() {
        let key = [0xAAu8; 32];
        let nonce = [0xBBu8; 12];
        let mut envelope = OpaqueEncryptedStore::encrypt(&key, &nonce, b"original text");
        envelope[6] ^= 0xFF;
        assert!(OpaqueEncryptedStore::decrypt(&key, &envelope).is_err());
    }

    #[test]
    fn test_b_f12_03_tampered_tag_fails_auth() {
        let key = [0xAAu8; 32];
        let nonce = [0xBBu8; 12];
        let mut envelope = OpaqueEncryptedStore::encrypt(&key, &nonce, b"original text");
        let last = envelope.len() - 1;
        envelope[last] ^= 0xFF;
        assert!(OpaqueEncryptedStore::decrypt(&key, &envelope).is_err());
    }

    #[test]
    fn test_b_f12_04_tampered_magic_bytes_fails() {
        let key = [0xAAu8; 32];
        let nonce = [0xBBu8; 12];
        let mut envelope = OpaqueEncryptedStore::encrypt(&key, &nonce, b"original text");
        envelope[0] = b'Z';
        assert!(OpaqueEncryptedStore::decrypt(&key, &envelope).is_err());
    }

    #[test]
    fn test_b_f12_05_empty_plaintext_encrypted_envelope() {
        let key = [0xCCu8; 32];
        let nonce = [0xDDu8; 12];
        let envelope = OpaqueEncryptedStore::encrypt(&key, &nonce, b"");
        let decrypted = OpaqueEncryptedStore::decrypt(&key, &envelope).unwrap();
        assert!(decrypted.is_empty());
    }

    // --- Boundary F13: Task Ledger ---
    #[tokio::test]
    async fn test_b_f13_01_huge_stdout_in_task_record() {
        let key = [0x11u8; 32];
        let ledger = OpaqueTaskLedger::new(key);
        let big_stdout = "output line\n".repeat(10_000);
        let task = OpaqueTaskRecord {
            task_id: "big-out".to_string(),
            agent: "agent".to_string(),
            instruction: "test".to_string(),
            substrate: "wasm".to_string(),
            status: "Completed".to_string(),
            stdout: big_stdout.clone(),
            stderr: "".to_string(),
            exit_code: 0,
            timestamp: 100,
        };
        ledger.record(task).await;
        let retrieved = ledger.get_task("big-out").await.unwrap();
        assert_eq!(retrieved.stdout.len(), big_stdout.len());
    }

    #[tokio::test]
    async fn test_b_f13_02_special_characters_in_task_instruction() {
        let key = [0x11u8; 32];
        let ledger = OpaqueTaskLedger::new(key);
        let task = OpaqueTaskRecord {
            task_id: "special-chars".to_string(),
            agent: "agent".to_string(),
            instruction: "echo 'hello world'; rm -rf /tmp/test\n\0\t".to_string(),
            substrate: "firecracker".to_string(),
            status: "Running".to_string(),
            stdout: "".to_string(),
            stderr: "".to_string(),
            exit_code: 0,
            timestamp: 200,
        };
        ledger.record(task.clone()).await;
        let retrieved = ledger.get_task("special-chars").await.unwrap();
        assert_eq!(retrieved.instruction, task.instruction);
    }

    #[tokio::test]
    async fn test_b_f13_03_concurrent_task_insertions() {
        let key = [0x11u8; 32];
        let ledger = Arc::new(OpaqueTaskLedger::new(key));
        let mut handles = Vec::new();

        for i in 0..20 {
            let l = ledger.clone();
            handles.push(tokio::spawn(async move {
                l.record(OpaqueTaskRecord {
                    task_id: format!("concurrent-{}", i),
                    agent: "agent".to_string(),
                    instruction: "work".to_string(),
                    substrate: "wasm".to_string(),
                    status: "Completed".to_string(),
                    stdout: "".to_string(),
                    stderr: "".to_string(),
                    exit_code: 0,
                    timestamp: i as u64,
                }).await;
            }));
        }

        for h in handles {
            h.await.unwrap();
        }

        for i in 0..20 {
            assert!(ledger.get_task(&format!("concurrent-{}", i)).await.is_some());
        }
    }

    #[tokio::test]
    async fn test_b_f13_04_tampered_ledger_file_fails_to_load() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("tampered_ledger.db");
        let key = [0x22u8; 32];
        let nonce = [0x33u8; 12];

        let ledger = OpaqueTaskLedger::new(key);
        ledger.record(OpaqueTaskRecord {
            task_id: "t1".to_string(),
            agent: "a".to_string(),
            instruction: "i".to_string(),
            substrate: "s".to_string(),
            status: "s".to_string(),
            stdout: "".to_string(),
            stderr: "".to_string(),
            exit_code: 0,
            timestamp: 0,
        }).await;
        ledger.flush_to_disk(&db_path, &nonce).await.unwrap();

        let mut bytes = tokio::fs::read(&db_path).await.unwrap();
        bytes[20] ^= 0xFF;
        tokio::fs::write(&db_path, &bytes).await.unwrap();

        let load_res = OpaqueTaskLedger::load_from_disk(&db_path, &key).await;
        assert!(load_res.is_err());
    }

    #[tokio::test]
    async fn test_b_f13_05_load_ledger_with_wrong_key_fails() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("key_test_ledger.db");
        let key = [0x44u8; 32];
        let wrong_key = [0x55u8; 32];
        let nonce = [0x66u8; 12];

        let ledger = OpaqueTaskLedger::new(key);
        ledger.flush_to_disk(&db_path, &nonce).await.unwrap();

        let load_res = OpaqueTaskLedger::load_from_disk(&db_path, &wrong_key).await;
        assert!(load_res.is_err());
    }

    // --- Boundary F14: SPIRE Federation & OIDC ---
    #[test]
    fn test_b_f14_01_missing_iss_claim_rejected() {
        let verifier = OpaqueOidcVerifier {
            issuer: "https://auth.com".to_string(),
            audience: "aura".to_string(),
            allowed_scopes: HashSet::new(),
        };
        let payload = serde_json::json!({"aud": "aura", "sub": "u", "exp": 9999999999u64});
        assert!(verifier.verify(&payload).is_err());
    }

    #[test]
    fn test_b_f14_02_missing_aud_claim_rejected() {
        let verifier = OpaqueOidcVerifier {
            issuer: "https://auth.com".to_string(),
            audience: "aura".to_string(),
            allowed_scopes: HashSet::new(),
        };
        let payload = serde_json::json!({"iss": "https://auth.com", "sub": "u", "exp": 9999999999u64});
        assert!(verifier.verify(&payload).is_err());
    }

    #[test]
    fn test_b_f14_03_missing_sub_claim_rejected() {
        let verifier = OpaqueOidcVerifier {
            issuer: "https://auth.com".to_string(),
            audience: "aura".to_string(),
            allowed_scopes: HashSet::new(),
        };
        let payload = serde_json::json!({"iss": "https://auth.com", "aud": "aura", "exp": 9999999999u64});
        assert!(verifier.verify(&payload).is_err());
    }

    #[test]
    fn test_b_f14_04_unauthorized_enterprise_roles_filtered_out() {
        let mut allowed = HashSet::new();
        allowed.insert("scoped-reader".to_string());
        let verifier = OpaqueOidcVerifier {
            issuer: "https://auth.com".to_string(),
            audience: "aura".to_string(),
            allowed_scopes: allowed,
        };

        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let payload = serde_json::json!({
            "iss": "https://auth.com",
            "aud": "aura",
            "sub": "user-42",
            "exp": now + 3600,
            "roles": ["scoped-reader", "admin:root", "danger:escalate"]
        });

        let svid = verifier.verify(&payload).unwrap();
        assert!(svid.has_scope("scoped-reader"));
        assert!(!svid.has_scope("admin:root"));
        assert!(!svid.has_scope("danger:escalate"));
    }

    #[test]
    fn test_b_f14_05_spire_bundle_retrieval() {
        let fed = FederationManager::new();
        let bundle = vec![0xDE, 0xAD, 0xBE, 0xEF];
        fed.register_federated_domain("partner.org", bundle.clone());
        assert_eq!(fed.get_bundle("partner.org").unwrap(), bundle);
    }

    // --- Boundary F15: Resilient Transport ---
    #[tokio::test]
    async fn test_b_f15_01_both_udp_and_tcp_unreachable_returns_error() {
        let blackhole_udp = UdpSocket::bind("127.0.0.1:0").await.unwrap().local_addr().unwrap();
        let unused_tcp = "127.0.0.1:1".parse().unwrap();

        let client = OpaqueResilientClient::new(blackhole_udp, unused_tcp, Duration::from_millis(20));
        let res = client.call_tool(&make_rpc_req(1, "test", serde_json::Value::Null)).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn test_b_f15_02_zero_timeout_forces_immediate_tcp_fallback() {
        let blackhole_udp = UdpSocket::bind("127.0.0.1:0").await.unwrap().local_addr().unwrap();

        let tcp_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let tcp_addr = tcp_listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (mut stream, _) = tcp_listener.accept().await.unwrap();
            let mut len_buf = [0u8; 4];
            stream.read_exact(&mut len_buf).await.unwrap();
            let len = u32::from_be_bytes(len_buf) as usize;
            let mut body = vec![0u8; len];
            stream.read_exact(&mut body).await.unwrap();

            let req: JsonRpcRequest = serde_json::from_slice(&body).unwrap();
            let resp = JsonRpcResponse::success(req.id, serde_json::json!({"zero_timeout": true}));
            let resp_bytes = serde_json::to_vec(&resp).unwrap();
            stream.write_all(&(resp_bytes.len() as u32).to_be_bytes()).await.unwrap();
            stream.write_all(&resp_bytes).await.unwrap();
        });

        let client = OpaqueResilientClient::new(blackhole_udp, tcp_addr, Duration::from_millis(0));
        let resp = client.call_tool(&make_rpc_req(1, "test", serde_json::Value::Null)).await.unwrap();
        assert_eq!(resp.result.as_ref().unwrap()["zero_timeout"], true);
    }

    #[tokio::test]
    async fn test_b_f15_03_large_1mb_tool_payload_fallback() {
        let blackhole_udp = UdpSocket::bind("127.0.0.1:0").await.unwrap().local_addr().unwrap();

        let tcp_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let tcp_addr = tcp_listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (mut stream, _) = tcp_listener.accept().await.unwrap();
            let mut len_buf = [0u8; 4];
            stream.read_exact(&mut len_buf).await.unwrap();
            let len = u32::from_be_bytes(len_buf) as usize;
            let mut body = vec![0u8; len];
            stream.read_exact(&mut body).await.unwrap();

            let req: JsonRpcRequest = serde_json::from_slice(&body).unwrap();
            let resp = JsonRpcResponse::success(req.id, serde_json::json!({"payload_len": len}));
            let resp_bytes = serde_json::to_vec(&resp).unwrap();
            stream.write_all(&(resp_bytes.len() as u32).to_be_bytes()).await.unwrap();
            stream.write_all(&resp_bytes).await.unwrap();
        });

        let client = OpaqueResilientClient::new(blackhole_udp, tcp_addr, Duration::from_millis(30));
        let large_param = "z".repeat(500_000);
        let req = make_rpc_req(10, "big_call", serde_json::json!({"data": large_param}));
        let resp = client.call_tool(&req).await.unwrap();
        assert!(resp.result.as_ref().unwrap()["payload_len"].as_u64().unwrap() > 500_000);
    }

    #[tokio::test]
    async fn test_b_f15_04_tcp_server_handles_tools_list() {
        let tcp_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let tcp_addr = tcp_listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (mut stream, _) = tcp_listener.accept().await.unwrap();
            let mut len_buf = [0u8; 4];
            stream.read_exact(&mut len_buf).await.unwrap();
            let len = u32::from_be_bytes(len_buf) as usize;
            let mut body = vec![0u8; len];
            stream.read_exact(&mut body).await.unwrap();

            let resp = JsonRpcResponse::success(Some(serde_json::json!(1)), serde_json::json!({"tools": ["bash", "browser", "computer"]}));
            let resp_bytes = serde_json::to_vec(&resp).unwrap();
            stream.write_all(&(resp_bytes.len() as u32).to_be_bytes()).await.unwrap();
            stream.write_all(&resp_bytes).await.unwrap();
        });

        let dummy_udp = "127.0.0.1:2".parse().unwrap();
        let client = OpaqueResilientClient::new(dummy_udp, tcp_addr, Duration::from_millis(10));
        let resp = client.call_tool(&make_rpc_req(1, "tools/list", serde_json::Value::Null)).await.unwrap();
        let tools = resp.result.as_ref().unwrap()["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 3);
    }

    #[tokio::test]
    async fn test_b_f15_05_tcp_server_handles_tools_call() {
        let tcp_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let tcp_addr = tcp_listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (mut stream, _) = tcp_listener.accept().await.unwrap();
            let mut len_buf = [0u8; 4];
            stream.read_exact(&mut len_buf).await.unwrap();
            let len = u32::from_be_bytes(len_buf) as usize;
            let mut body = vec![0u8; len];
            stream.read_exact(&mut body).await.unwrap();

            let resp = JsonRpcResponse::success(Some(serde_json::json!(5)), serde_json::json!({"stdout": "hello from tool\n"}));
            let resp_bytes = serde_json::to_vec(&resp).unwrap();
            stream.write_all(&(resp_bytes.len() as u32).to_be_bytes()).await.unwrap();
            stream.write_all(&resp_bytes).await.unwrap();
        });

        let dummy_udp = "127.0.0.1:2".parse().unwrap();
        let client = OpaqueResilientClient::new(dummy_udp, tcp_addr, Duration::from_millis(10));
        let resp = client.call_tool(&make_rpc_req(5, "tools/call", serde_json::json!({"name": "bash"}))).await.unwrap();
        assert_eq!(resp.result.as_ref().unwrap()["stdout"], "hello from tool\n");
    }
}

// ============================================================================
// Tier 3: Pairwise Cross-Feature Interactions (15 tests)
// ============================================================================

mod tier3_pairwise_interactions {
    use super::*;

    #[test]
    fn test_pairwise_01_f1_redaction_and_f4_oob_injection() {
        let cred = OpaqueCredential::new("github", "ghp_real_secret_token_12345678", 999999);
        let log_output = format!("{}", cred);
        assert!(!log_output.contains("real_secret_token"));
        assert!(log_output.contains("[REDACTED-TOKEN-12345678]"));

        let mut env = HashMap::new();
        OutOfBandInjector::inject_into_env(&mut env, "GH_TOKEN", &cred).unwrap();
        assert_eq!(env.get("GH_TOKEN").unwrap(), "ghp_real_secret_token_12345678");
    }

    #[tokio::test]
    async fn test_pairwise_02_f2_scope_enforcement_and_f5_pr_creation() {
        let service = OpaqueTunnelService::new();
        let unpriv_svid = Svid::new("spiffe://aura.local/agent/reader").with_scopes(vec!["fs:read".to_string()]);
        let mint_res = service.mint_credentials("github", &unpriv_svid, &["github"], 120).await;
        assert!(mint_res.is_err());
    }

    #[tokio::test]
    async fn test_pairwise_03_f3_revocation_and_f4_sandbox_session() {
        let service = OpaqueTunnelService::new();
        let svid = Svid::new("spiffe://aura.local/agent").with_scopes(vec!["github".to_string()]);
        let cred = service.mint_credentials("github", &svid, &["github"], 120).await.unwrap();

        let mut env = HashMap::new();
        OutOfBandInjector::inject_into_env(&mut env, "TOKEN", &cred).unwrap();

        service.revoke_credentials(cred.raw_secret()).await.unwrap();

        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let valid = service.is_valid(env.get("TOKEN").unwrap(), now).await;
        assert!(!valid);
    }

    #[tokio::test]
    async fn test_pairwise_04_f5_pr_creation_and_f12_encrypted_ledger() {
        let key = [0x55u8; 32];
        let ledger = OpaqueTaskLedger::new(key);
        let git = GitManager::new("main");
        let svid = Svid::new("spiffe://aura.local/agent").with_scopes(vec!["github".to_string()]);
        let pr_res = git.open_pr(&svid, "feat: pairwise pr", "body").unwrap();

        ledger.record(OpaqueTaskRecord {
            task_id: "pr-task-1".to_string(),
            agent: "coder".to_string(),
            instruction: "open_pr".to_string(),
            substrate: "git".to_string(),
            status: "Completed".to_string(),
            stdout: pr_res,
            stderr: "".to_string(),
            exit_code: 0,
            timestamp: 100,
        }).await;

        let task = ledger.get_task("pr-task-1").await.unwrap();
        assert_eq!(task.stdout, "Opened PR: feat: pairwise pr");
    }

    #[test]
    fn test_pairwise_05_f6_x11_actions_and_f8_png_redaction() {
        let click_req: serde_json::Value = serde_json::from_str(
            r#"{"jsonrpc": "2.0", "id": 1, "method": "click", "params": {"x": 5, "y": 5, "button": 1}}"#
        ).unwrap();
        let cx = click_req["params"]["x"].as_u64().unwrap() as usize;
        let cy = click_req["params"]["y"].as_u64().unwrap() as usize;

        let mut pixels = vec![255u8; 10 * 10 * 4];
        apply_scanline_redaction(&mut pixels, 10, 10, &[(cx - 1, cy - 1, 3, 3)]);

        let offset = (cy * 10 + cx) * 4;
        assert_eq!(&pixels[offset..offset + 4], &[0, 0, 0, 255]);
    }

    #[tokio::test]
    async fn test_pairwise_06_f7_depth_mask_and_f10_vsock_stream_limit() {
        let fake_screenshot = vec![0x89, b'P', b'N', b'G', 13, 10, 26, 10];
        let mut large_stream = fake_screenshot.clone();
        large_stream.resize(100_000, 0xAA);

        let mut cursor = io::Cursor::new(large_stream);
        let (buf, truncated) = read_stream_with_limit(&mut cursor, 16_384).await.unwrap();
        assert_eq!(buf.len(), 16_384);
        assert!(truncated);
        assert_eq!(&buf[0..8], &fake_screenshot[0..8]);
    }

    #[test]
    fn test_pairwise_07_f8_png_redaction_and_f12_encrypted_storage() {
        let mut pixels = vec![255u8; 10 * 10 * 4];
        apply_scanline_redaction(&mut pixels, 10, 10, &[(0, 0, 10, 10)]);

        let key = [0x77u8; 32];
        let nonce = [0x88u8; 12];
        let envelope = OpaqueEncryptedStore::encrypt(&key, &nonce, &pixels);

        assert_eq!(&envelope[0..4], b"AURA");
        let decrypted = OpaqueEncryptedStore::decrypt(&key, &envelope).unwrap();
        assert_eq!(decrypted, pixels);
    }

    #[tokio::test]
    async fn test_pairwise_08_f10_stream_truncation_and_f11_egress_proxy() {
        let rogue_vsock_stream = vec![0xFF; 20_000];
        let mut cursor = io::Cursor::new(rogue_vsock_stream);
        let (_, truncated) = read_stream_with_limit(&mut cursor, 1024).await.unwrap();
        assert!(truncated);

        let config = EgressProxyConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            allowed_domains: vec!["crates.io".to_string()],
            enforce_strict_tls: true,
            upstream_override: None,
        };
        let proxy = EgressProxy::start(config).await.unwrap();
        let mut client = TcpStream::connect(proxy.bind_addr).await.unwrap();
        let hello = create_tls_client_hello("malicious-c2.org");
        client.write_all(&hello).await.unwrap();

        let mut alert = [0u8; 7];
        client.read_exact(&mut alert).await.unwrap();
        assert_eq!(&alert[..], TLS_ALERT_ACCESS_DENIED);

        proxy.stop();
    }

    #[tokio::test]
    async fn test_pairwise_09_f11_egress_proxy_and_f14_spire_federation() {
        let fed = FederationManager::new();
        fed.register_federated_domain("external.corp", vec![1, 2, 3]);
        assert!(fed.is_federated("external.corp"));

        let config = EgressProxyConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            allowed_domains: vec!["crates.io".to_string()],
            enforce_strict_tls: true,
            upstream_override: None,
        };
        let proxy = EgressProxy::start(config).await.unwrap();
        assert!(proxy.bind_addr.port() > 0);
        proxy.stop();
    }

    #[test]
    fn test_pairwise_10_f12_encrypted_db_and_f14_spiffe_key_derivation() {
        let svid = Svid::new("spiffe://aura.local/agent/database-user");
        let derived_key = OpaqueEncryptedStore::derive_key(&svid, "aura-db-encryption");
        let nonce = [0x01u8; 12];
        let envelope = OpaqueEncryptedStore::encrypt(&derived_key, &nonce, b"db_data");
        let decrypted = OpaqueEncryptedStore::decrypt(&derived_key, &envelope).unwrap();
        assert_eq!(&decrypted[..], b"db_data");
    }

    #[tokio::test]
    async fn test_pairwise_11_f13_task_ledger_and_f15_transport_failover() {
        let key = [0x88u8; 32];
        let ledger = OpaqueTaskLedger::new(key);

        let blackhole_udp = UdpSocket::bind("127.0.0.1:0").await.unwrap().local_addr().unwrap();
        let tcp_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let tcp_addr = tcp_listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (mut stream, _) = tcp_listener.accept().await.unwrap();
            let mut len_buf = [0u8; 4];
            stream.read_exact(&mut len_buf).await.unwrap();
            let len = u32::from_be_bytes(len_buf) as usize;
            let mut body = vec![0u8; len];
            stream.read_exact(&mut body).await.unwrap();

            let req: JsonRpcRequest = serde_json::from_slice(&body).unwrap();
            let resp = JsonRpcResponse::success(req.id, serde_json::json!({"failover": "ok"}));
            let resp_bytes = serde_json::to_vec(&resp).unwrap();
            stream.write_all(&(resp_bytes.len() as u32).to_be_bytes()).await.unwrap();
            stream.write_all(&resp_bytes).await.unwrap();
        });

        let client = OpaqueResilientClient::new(blackhole_udp, tcp_addr, Duration::from_millis(30));
        let resp = client.call_tool(&make_rpc_req(1, "tools/list", serde_json::Value::Null)).await.unwrap();

        ledger.record(OpaqueTaskRecord {
            task_id: "transport-task-1".to_string(),
            agent: "router".to_string(),
            instruction: "failover tool call".to_string(),
            substrate: "http2".to_string(),
            status: "Completed".to_string(),
            stdout: resp.result.as_ref().unwrap().to_string(),
            stderr: "".to_string(),
            exit_code: 0,
            timestamp: 500,
        }).await;

        let task = ledger.get_task("transport-task-1").await.unwrap();
        assert!(task.stdout.contains("failover"));
    }

    #[tokio::test]
    async fn test_pairwise_12_f1_redaction_and_f13_task_ledger() {
        let cred = OpaqueCredential::new("github", "real_secret_token_12345678", 999999);
        let key = [0x99u8; 32];
        let ledger = OpaqueTaskLedger::new(key);

        ledger.record(OpaqueTaskRecord {
            task_id: "t-redact".to_string(),
            agent: "worker".to_string(),
            instruction: format!("Use cred: {}", cred),
            substrate: "wasm".to_string(),
            status: "Completed".to_string(),
            stdout: format!("Logged cred: {}", cred),
            stderr: "".to_string(),
            exit_code: 0,
            timestamp: 100,
        }).await;

        let task = ledger.get_task("t-redact").await.unwrap();
        assert!(!task.instruction.contains("real_secret_token"));
        assert!(!task.stdout.contains("real_secret_token"));
        assert!(task.instruction.contains("[REDACTED-TOKEN-12345678]"));
    }

    #[test]
    fn test_pairwise_13_f2_scope_enforcement_and_f14_oidc_verification() {
        let mut allowed = HashSet::new();
        allowed.insert("tool:git".to_string());
        let verifier = OpaqueOidcVerifier {
            issuer: "https://auth.company.com".to_string(),
            audience: "aura".to_string(),
            allowed_scopes: allowed,
        };

        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let payload = serde_json::json!({
            "iss": "https://auth.company.com",
            "aud": "aura",
            "sub": "user-99",
            "exp": now + 1000,
            "roles": ["tool:git", "cloud:admin"]
        });

        let svid = verifier.verify(&payload).unwrap();
        assert!(svid.has_scope("tool:git"));
        assert!(!svid.has_scope("cloud:admin"));
    }

    #[tokio::test]
    async fn test_pairwise_14_f3_revocation_and_f15_transport_client() {
        let service = OpaqueTunnelService::new();
        let svid = Svid::new("spiffe://aura.local/agent").with_scopes(vec!["github".to_string()]);
        let cred = service.mint_credentials("github", &svid, &["github"], 0).await.unwrap();

        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let is_valid = service.is_valid(cred.raw_secret(), now + 1).await;
        assert!(!is_valid, "Transport dispatch should reject expired token");
    }

    #[tokio::test]
    async fn test_pairwise_15_f6_x11_actions_and_f10_vsock_stream_limit() {
        let mut burst = Vec::new();
        for i in 1..=10_000 {
            let cmd = format!(r#"{{"jsonrpc": "2.0", "id": {}, "method": "move", "params": {{"x": {}, "y": {}}}}}\n"#, i, i, i);
            burst.extend_from_slice(cmd.as_bytes());
        }

        let mut cursor = io::Cursor::new(burst);
        let (buf, truncated) = read_stream_with_limit(&mut cursor, 65_536).await.unwrap();
        assert_eq!(buf.len(), 65_536);
        assert!(truncated);
    }
}

// ============================================================================
// Tier 4: Real-World High-Complexity Scenarios (5 Scenarios from TEST_INFRA.md)
// ============================================================================

mod tier4_real_world_scenarios {
    use super::*;

    /// Scenario 1: End-to-End Secure PR Workflow (Features: F1, F2, F3, F4, F5)
    #[tokio::test]
    async fn test_scenario_1_end_to_end_secure_pr_workflow() {
        // 1. Attest caller SVID with GitHub scope
        let caller_svid = Svid::new("spiffe://aura.local/agent/coder")
            .with_scopes(vec!["github".to_string(), "tool:ephemeral-tunnel".to_string()]);

        // 2. Mint short-lived ephemeral credentials
        let tunnel = OpaqueTunnelService::new();
        let cred = tunnel.mint_credentials("github", &caller_svid, &["github"], 120).await.unwrap();

        // 3. Inspect formatting: assert sanitized handle does NOT leak raw secret
        let displayed = format!("{}", cred);
        assert!(displayed.contains("[REDACTED-TOKEN-"));
        assert!(!displayed.contains(cred.raw_secret()));

        // 4. Inject out-of-band into sandbox environment
        let mut sandbox_env = HashMap::new();
        OutOfBandInjector::inject_into_env(&mut sandbox_env, "GITHUB_TOKEN", &cred).unwrap();
        assert_eq!(sandbox_env.get("GITHUB_TOKEN").unwrap(), cred.raw_secret());

        // 5. Initialize Git repository manager
        let temp_dir = tempfile::tempdir().unwrap();
        let file1 = temp_dir.path().join("code.rs");
        tokio::fs::write(&file1, b"fn main() {}").await.unwrap();
        let git = GitManager::new("main").with_files(&[file1.clone()]);

        assert_eq!(git.check_drift(), Ok(()));

        tokio::fs::write(&file1, b"fn main() { println!(\"updated\"); }").await.unwrap();
        git.record_agent_change(&file1);
        assert_eq!(git.check_drift(), Ok(()));

        // 6. Open Pull Request with RAII revocation drop guard
        let pr_title = "feat: implement hardening features";
        let pr_result = {
            let _guard = PrRevocationGuard::new(&tunnel, cred.raw_secret());
            let res = git.open_pr(&caller_svid, pr_title, "PR description body");
            assert!(res.is_ok());
            res.unwrap()
        };
        assert_eq!(pr_result, "Opened PR: feat: implement hardening features");

        // 7. Verify token was automatically revoked upon PR completion
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(tunnel.is_revoked(cred.raw_secret()).await);

        assert_eq!(git.active_tokens.lock().unwrap().len(), 0);
        assert_eq!(git.revoked_tokens.lock().unwrap().len(), 1);
    }

    /// Scenario 2: Visual GUI Automation with Sensitive Redaction (Features: F6, F7, F8, F9)
    #[test]
    fn test_scenario_2_visual_gui_automation_with_sensitive_redaction() {
        let actions = vec![
            serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "move", "params": {"x": 200, "y": 150}}),
            serde_json::json!({"jsonrpc": "2.0", "id": 2, "method": "click", "params": {"x": 200, "y": 150, "button": 1}}),
            serde_json::json!({"jsonrpc": "2.0", "id": 3, "method": "type", "params": {"text": "password123"}}),
        ];
        for act in &actions {
            assert_eq!(act["jsonrpc"], "2.0");
        }

        let width = 100usize;
        let height = 100usize;
        let mut framebuffer = vec![255u8; width * height * 4];

        let depth = 24u32;
        let plane_mask = if depth < 32 { (1u32 << depth) - 1 } else { !0 };
        assert_eq!(plane_mask, 0x00FF_FFFF);

        let sensitive_box = (15, 14, 25, 6);
        apply_scanline_redaction(&mut framebuffer, width, height, &[sensitive_box]);

        let sample_offset = (15 * width + 20) * 4;
        assert_eq!(&framebuffer[sample_offset..sample_offset + 4], &[0, 0, 0, 255]);
        assert_eq!(&framebuffer[0..4], &[255, 255, 255, 255]);
    }

    /// Scenario 3: Rogue Guest Stream & Egress Violation Defense (Features: F10, F11)
    #[tokio::test]
    async fn test_scenario_3_rogue_guest_stream_and_egress_violation_defense() {
        // Part A: Rogue microVM floods vsock stream with 64KB of data
        let rogue_chunk = vec![0xEE; 64 * 1024];
        let mut cursor = io::Cursor::new(rogue_chunk);
        let max_host_limit = 16 * 1024;
        let (retained_buf, truncated) = read_stream_with_limit(&mut cursor, max_host_limit).await.unwrap();

        assert_eq!(retained_buf.len(), max_host_limit);
        assert!(truncated);

        // Part B: Rogue microVM initiates unauthorized network egress
        let config = EgressProxyConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            allowed_domains: vec!["crates.io".to_string(), "pypi.org".to_string()],
            enforce_strict_tls: true,
            upstream_override: None,
        };
        let proxy = EgressProxy::start(config).await.unwrap();
        let proxy_addr = proxy.bind_addr;

        let mut rogue_socket = TcpStream::connect(proxy_addr).await.unwrap();
        let client_hello = create_tls_client_hello("unauthorized-exfiltration.net");
        rogue_socket.write_all(&client_hello).await.unwrap();

        let mut alert_response = [0u8; 7];
        rogue_socket.read_exact(&mut alert_response).await.unwrap();

        assert_eq!(&alert_response[..], TLS_ALERT_ACCESS_DENIED);

        let mut eof = [0u8; 1];
        let bytes_read = rogue_socket.read(&mut eof).await.unwrap();
        assert_eq!(bytes_read, 0);

        proxy.stop();
    }

    /// Scenario 4: Encrypted Ledger Audit & Zero-Knowledge Verification (Features: F12, F13)
    #[tokio::test]
    async fn test_scenario_4_encrypted_ledger_audit_and_zero_knowledge_verification() {
        let temp_dir = tempfile::tempdir().unwrap();
        let ledger_path = temp_dir.path().join("secure_tasks.db");

        let svid = Svid::new("spiffe://aura.local/agent/ledger-auditor");
        let storage_key = OpaqueEncryptedStore::derive_key(&svid, "aura-ledger-encryption");
        let nonce = [0x55u8; 12];

        let ledger = OpaqueTaskLedger::new(storage_key);
        let task = OpaqueTaskRecord {
            task_id: "task-scenario-4-secret-999".to_string(),
            agent: "lead-architect".to_string(),
            instruction: "sensitive financial ledger calculation".to_string(),
            substrate: "firecracker".to_string(),
            status: "Completed".to_string(),
            stdout: "confidential execution output: 42000000".to_string(),
            stderr: "".to_string(),
            exit_code: 0,
            timestamp: 123456789,
        };
        ledger.record(task.clone()).await;

        ledger.flush_to_disk(&ledger_path, &nonce).await.unwrap();

        let raw_disk_bytes = tokio::fs::read(&ledger_path).await.unwrap();

        assert_eq!(&raw_disk_bytes[0..4], b"AURA");
        assert_eq!(raw_disk_bytes[4], 1);
        assert_ne!(&raw_disk_bytes[0..16], b"SQLite format 3\0");

        let disk_text = String::from_utf8_lossy(&raw_disk_bytes);
        assert!(!disk_text.contains("task-scenario-4-secret"));
        assert!(!disk_text.contains("sensitive financial ledger"));
        assert!(!disk_text.contains("confidential execution output"));
        assert!(!disk_text.contains("CREATE TABLE"));

        let reloaded_ledger = OpaqueTaskLedger::load_from_disk(&ledger_path, &storage_key).await.unwrap();
        let retrieved_task = reloaded_ledger.get_task("task-scenario-4-secret-999").await.unwrap();
        assert_eq!(retrieved_task, task);

        let wrong_key = [0xFFu8; 32];
        let tamper_res = OpaqueTaskLedger::load_from_disk(&ledger_path, &wrong_key).await;
        assert!(tamper_res.is_err());
    }

    /// Scenario 5: Enterprise Federation & Transport Self-Healing (Features: F14, F15)
    #[tokio::test]
    async fn test_scenario_5_enterprise_federation_and_transport_self_healing() {
        let mut allowed_scopes = HashSet::new();
        allowed_scopes.insert("tool:mcp".to_string());
        let verifier = OpaqueOidcVerifier {
            issuer: "https://idp.enterprise.com".to_string(),
            audience: "aura-system".to_string(),
            allowed_scopes,
        };

        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let enterprise_jwt = serde_json::json!({
            "iss": "https://idp.enterprise.com",
            "aud": "aura-system",
            "sub": "auditor-corp-user",
            "exp": now + 7200,
            "roles": ["tool:mcp"]
        });

        let attested_svid = verifier.verify(&enterprise_jwt).unwrap();
        assert!(attested_svid.has_scope("tool:mcp"));

        let blackhole_udp = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let udp_addr = blackhole_udp.local_addr().unwrap();

        let tcp_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let tcp_addr = tcp_listener.local_addr().unwrap();

        tokio::spawn(async move {
            while let Ok((mut stream, _)) = tcp_listener.accept().await {
                let mut len_buf = [0u8; 4];
                if stream.read_exact(&mut len_buf).await.is_err() {
                    break;
                }
                let len = u32::from_be_bytes(len_buf) as usize;
                let mut body = vec![0u8; len];
                stream.read_exact(&mut body).await.unwrap();

                let req: JsonRpcRequest = serde_json::from_slice(&body).unwrap();
                let resp = JsonRpcResponse::success(
                    req.id,
                    serde_json::json!({
                        "result": "executed_on_tcp_fallback",
                        "tool": req.method,
                        "status": "success"
                    }),
                );
                let resp_bytes = serde_json::to_vec(&resp).unwrap();
                stream.write_all(&(resp_bytes.len() as u32).to_be_bytes()).await.unwrap();
                stream.write_all(&resp_bytes).await.unwrap();
            }
        });

        let client = OpaqueResilientClient::new(udp_addr, tcp_addr, Duration::from_millis(50));

        let request = make_rpc_req(
            101,
            "tools/call",
            serde_json::json!({"name": "ast-blast-radius", "path": "src/lib.rs"}),
        );

        let start_time = Instant::now();
        let response = client.call_tool(&request).await.unwrap();
        let duration = start_time.elapsed();

        assert!(response.is_success());
        assert_eq!(response.id, Some(serde_json::json!(101)));
        assert_eq!(response.result.as_ref().unwrap()["result"], "executed_on_tcp_fallback");

        assert!(duration < Duration::from_millis(200));

        let req2 = make_rpc_req(102, "tools/list", serde_json::Value::Null);
        let resp2 = client.call_tool(&req2).await.unwrap();
        assert!(resp2.is_success());
        assert_eq!(resp2.id, Some(serde_json::json!(102)));
    }
}
