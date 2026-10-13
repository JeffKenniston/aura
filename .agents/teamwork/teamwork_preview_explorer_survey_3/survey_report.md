# Survey Report: Phase M5 Hardening (R3 & R4)

**Surveyor**: `teamwork_preview_explorer_survey_3`  
**Date**: 2026-10-11  
**Target Milestone**: Phase M5 (Hardening)  
**Requirements**: 
- **R3**: Microkernel Zero-Trust Security & Storage Encryption
- **R4**: SPIRE Identity Federation & Transport Fallback Resilience

---

## Executive Summary

This survey report provides a forensic inspection of the Aura codebase (`aura-core`, hypervisor, transport, identity/spiffe, and knowledge subsystems) to establish the architectural blueprint and implementation plan for Phase M5 requirements R3 and R4.

### High-Level Status Matrix

| ID | Requirement Area | Current Status | Existing Foundation | Key Gaps to Close |
|---|---|---|---|---|
| **R3.1** | microVM Vsock Stream Buffer Limits & Truncation | **Partial** | `start_screenshot_listener` binds Unix sockets for vsock streaming (`firecracker.rs:343-410`) | Reads unbounded streams via `read_to_end`; missing configurable buffer limit and truncation on rogue payload |
| **R3.2** | MicroVM Network Egress Transparent TLS Enforcement | **Substantially Complete** | `EgressProxy` in `hypervisor/proxy.rs` implements RFC-compliant SNI extraction, registry allowlisting, and TLS Fatal Alert 49 termination | Missing module re-export in `hypervisor/mod.rs` and explicit runtime binding helper in `FirecrackerVm` |
| **R3.3** | SQLite & Task Ledger AES-256-GCM Storage Encryption tied to SPIFFE | **Missing** | `rusqlite` bundled in `aura-core`, `SemanticCache` uses plaintext SQLite; `ring::aead` available in dependency graph | Missing `storage` module with AES-256-GCM encryption at rest, SPIFFE key derivation, raw ciphertext verification, and `TaskLedger` struct |
| **R4.1** | SPIRE Identity Federation & Enterprise OIDC Token Authentication | **Partial** | `FederationManager` in `identity/mod.rs:666-694` holds federated trust domain bundles; `MtlsEnforcer` checks trust domain | Missing enterprise OIDC token parser/verifier, SPIRE federation claim-to-scope mappings, and non-escalation enforcement |
| **R4.2** | Transport 1-RTT UDP Failure Detection & Silent HTTP/2 TCP Fallback | **Partial** | `Http3Transport` in `transport/http3.rs` binds UDP and TCP listeners; `McpServer` handles JSON-RPC & HTTP | TCP listener returns dummy Alt-Svc without executing MCP; missing client-side 1-RTT UDP failure detector, silent TCP failover, and in-flight call replay |

---

## 1. Deep Dive: Requirement R3 — Microkernel Zero-Trust Security & Storage Encryption

### 1.1 Vsock Stream Buffer Limits & Truncation (R3.1)

#### 1.1.1 Current Implementation & Code Citations
- **Source File**: `aura-core/src/hypervisor/firecracker.rs`
- **Existing Functionality**:
  - `start_screenshot_listener` (`lines 343-410`) creates a host-side Unix domain socket listener representing the virtio-vsock bridge (`<vsock_uds_path>_<port>`).
  - Line 382–386:
    ```rust
    let mut buffer = Vec::new();
    if let Err(e) = stream.read_to_end(&mut buffer).await {
        eprintln!("Failed reading vsock screenshot stream: {}", e);
        return;
    }
    ```
  - `extract_png_bytes` (`lines 237-275`) validates PNG magic bytes and length prefixes.
  - `execute` (`lines 883-895`) handles bash session vsock communication and limits the handshake to 256 bytes, but uses `read_line` for responses without an explicit buffer ceiling.

#### 1.1.2 Identified Architectural Deficiencies
1. **Unbounded Host Memory Allocation**: `read_to_end` continues allocating heap memory as long as the guest streams bytes. A compromised microVM can stream gigabytes of junk, causing host OOM panics or memory leaks.
2. **Missing Truncation on Violation**: The system must enforce a configurable buffer limit (e.g. 16 MiB default for screenshots, customizable down to kilobytes for test harnesses). When a guest exceeds this limit, the stream must be **instantly truncated** without panicking or allocating additional memory.
3. **Graceful Zero-Trust Rejection**: The truncated payload must be handled deterministically (e.g., failed PNG magic validation) without crashing the listener task.

#### 1.1.3 Interface Contract & Proposed Structures
```rust
/// Default memory limit for incoming vsock stream buffers (16 MiB).
pub const DEFAULT_MAX_VSOCK_STREAM_BYTES: usize = 16 * 1024 * 1024;

/// Configuration parameters for vsock stream listeners.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VsockStreamConfig {
    pub max_buffer_bytes: usize,
}

impl Default for VsockStreamConfig {
    fn default() -> Self {
        Self {
            max_buffer_bytes: DEFAULT_MAX_VSOCK_STREAM_BYTES,
        }
    }
}

/// Reads an incoming asynchronous byte stream up to `limit` bytes.
/// If the stream attempts to emit more bytes than `limit`, the payload is instantly
/// truncated at exactly `limit` bytes, discarding remaining bytes and terminating reading
/// without panicking or leaking heap memory.
pub async fn read_stream_with_limit<R: tokio::io::AsyncRead + Unpin>(
    reader: &mut R,
    limit: usize,
) -> Result<(Vec<u8>, bool), std::io::Error>;

/// Starts a vsock screenshot listener enforcing configurable memory buffer limits.
pub async fn start_screenshot_listener_with_config(
    socket_path: PathBuf,
    artifacts_dir: PathBuf,
    config: VsockStreamConfig,
) -> Result<ScreenshotListenerHandle, Box<dyn std::error::Error + Send + Sync>>;
```

---

### 1.2 MicroVM Network Egress Transparent TLS Enforcement (R3.2)

#### 1.2.1 Current Implementation & Code Citations
- **Source File**: `aura-core/src/hypervisor/proxy.rs` (`lines 1-646`)
- **Configuration File**: `aura-core/src/config/mod.rs` (`lines 264-281`, `HypervisorConfig::proxy: EgressProxyConfig`)
- **Existing Functionality**:
  - `DEFAULT_ALLOWED_DOMAINS` (`lines 9-30`) allowlists `crates.io`, `pypi.org`, `deb.debian.org`, `archive.ubuntu.com`, `dl-cdn.alpinelinux.org`, `github.com`, etc.
  - `TLS_ALERT_ACCESS_DENIED` (`lines 33-39`) defines TLS Fatal Alert 49 (Access Denied): `[0x15, 0x03, 0x03, 0x00, 0x02, 0x02, 0x31]`.
  - `EgressProxy::extract_sni` (`lines 136-234`) parses ClientHello RFC 5246 / RFC 6066 / RFC 8446 packets in pure Rust with strict bounds checking.
  - `EgressProxy::is_domain_allowed` (`lines 107-132`) handles exact matches, wildcard subdomains, and rejects prefix spoofing (`evil-crates.io`).
  - `EgressProxy::handle_connection` (`lines 311-419`):
    - Blocks unauthorized TLS handshakes by sending `TLS_ALERT_ACCESS_DENIED` and shutting down the connection.
    - Proxies authorized domains via `tokio::io::copy_bidirectional`.
    - Handles explicit HTTP CONNECT proxying (rejects unauthorized with `403 Forbidden`).
    - Enforces default-deny on non-TLS / non-proxy traffic.
  - Unit tests (`lines 423-645`): 100% passing test coverage for SNI extraction, allowlist matching, TLS fatal alert termination, upstream proxying, and HTTP CONNECT filtering.

#### 1.2.2 Key Findings & Missing Wiring
1. **Module Re-export Missing**: `aura-core/src/hypervisor/mod.rs` declares `pub mod proxy;` but does not re-export `EgressProxy`, `EgressProxyConfig`, `EgressProxyHandle`, etc.
2. **MicroVM Attachment Hook**: `FirecrackerVm` in `firecracker.rs` lacks an explicit method to link or verify the egress proxy configuration during microVM lifecycle provisioning.

---

### 1.3 SQLite & Task Ledger AES-256-GCM Storage Encryption (R3.3)

#### 1.3.1 Current Implementation & Code Citations
- **Source Files**: 
  - `aura-core/src/knowledge/cache.rs` (`lines 97-107`, `SemanticCache::open_file` uses unencrypted `Connection::open(path)`)
  - `aura-core/src/identity/mod.rs` (`lines 524-563`, `Svid` stores `cert_chain_der` and `private_key_der`)
- **Existing Dependencies**:
  - `rusqlite = { version = "0.31", features = ["bundled"] }` in `aura-core/Cargo.toml`.
  - `blake3 = "1.5"` for cryptographic key derivation.
  - `ring = "0.17"` present in `Cargo.lock` providing standard `ring::aead::AES_256_GCM`.

#### 1.3.2 Identified Architectural Deficiencies
1. **No Encryption at Rest**: SQLite database files on disk are standard plaintext format starting with `SQLite format 3\0` (offset 0..16). File inspection exposes schema (`CREATE TABLE ...`) and cached prompt strings.
2. **No Task Ledger**: There is currently no persistent `TaskLedger` struct or SQLite table in `aura-core` recording task dispatch, status transitions, stdout/stderr, and completion metadata.
3. **No Attested Identity Key Binding**: Persistent storage is not cryptographically bound to an attested SPIFFE identity key.

#### 1.3.3 Interface Contract & Proposed Structures
Create `aura-core/src/storage/` module with `encrypted.rs` and `ledger.rs`:

```rust
// aura-core/src/storage/encrypted.rs

/// Header magic bytes for encrypted Aura storage containers.
pub const ENCRYPTED_STORAGE_MAGIC: &[u8; 4] = b"AURA";
pub const ENCRYPTED_STORAGE_VERSION: u8 = 1;
pub const NONCE_LEN: usize = 12;
pub const TAG_LEN: usize = 16;

/// Pure-Rust AES-256-GCM container encrypting SQLite databases at rest.
pub struct EncryptedDatabase {
    path: PathBuf,
    conn: Arc<Mutex<rusqlite::Connection>>,
    derived_key: [u8; 32],
}

impl EncryptedDatabase {
    /// Opens or creates an encrypted SQLite database bound to an attested SPIFFE SVID.
    pub fn open_or_create<P: AsRef<Path>>(
        path: P,
        svid: &Svid,
    ) -> Result<Self, StorageError>;

    /// Flushes the active in-memory SQLite state to disk as authenticated AES-256-GCM ciphertext.
    pub fn flush_to_disk(&self) -> Result<(), StorageError>;

    /// Direct access to the internal rusqlite Connection.
    pub fn connection(&self) -> Arc<Mutex<rusqlite::Connection>>;
}

// Key Derivation in aura-core/src/identity/mod.rs
impl Svid {
    /// Derives a deterministic 256-bit symmetric storage key from the attested SPIFFE identity.
    pub fn derive_storage_key(&self, context: &str) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new_derive_key(context);
        hasher.update(self.id.as_bytes());
        if let Some(ref priv_key) = self.private_key_der {
            hasher.update(priv_key);
        }
        *hasher.finalize().as_bytes()
    }
}
```

```rust
// aura-core/src/storage/ledger.rs

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRecord {
    pub task_id: String,
    pub agent: String,
    pub instruction: String,
    pub substrate: String,
    pub status: TaskStatus,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub created_at: u64,
    pub updated_at: u64,
}

/// Persistent task ledger backed by an encrypted SQLite database at rest.
pub struct TaskLedger {
    db: EncryptedDatabase,
}

impl TaskLedger {
    pub fn open_or_create<P: AsRef<Path>>(path: P, svid: &Svid) -> Result<Self, StorageError>;
    pub fn record_dispatch(&self, task: &TaskPayload) -> Result<(), StorageError>;
    pub fn record_completion(&self, task_id: &str, result: &TaskResult) -> Result<(), StorageError>;
    pub fn get_task(&self, task_id: &str) -> Result<Option<TaskRecord>, StorageError>;
    pub fn list_tasks(&self) -> Result<Vec<TaskRecord>, StorageError>;
    pub fn flush(&self) -> Result<(), StorageError>;
}
```

#### 1.3.4 Raw Disk Byte Inspection Verification Criteria
1. Reading `std::fs::read(path)` must confirm:
   - Does NOT start with `b"SQLite format 3\0"`.
   - Starts with `b"AURA"` magic and 1-byte version.
   - Searching byte slice for `"CREATE TABLE"`, `"semantic_cache"`, `"tasks"`, `"instruction"`, or task IDs returns 0 occurrences.
2. Opening the file with a different SVID or modified ciphertext fails with an authentication error (GCM tag verification failure).

---

## 2. Deep Dive: Requirement R4 — SPIRE Identity Federation & Transport Fallback Resilience

### 2.1 SPIRE Identity Federation & Enterprise OIDC Token Verification (R4.1)

#### 2.1.1 Current Implementation & Code Citations
- **Source File**: `aura-core/src/identity/mod.rs`
- **Existing Functionality**:
  - `FederationManager` (`lines 670-694`):
    ```rust
    pub struct FederationManager {
        federated_domains: dashmap::DashMap<String, Vec<u8>>,
    }
    ```
    Stores trust domain bundle DER mappings.
  - `IdentityManager::validate_peer` (`lines 614-627`): checks `peer_domain == self.trust_domain || self.federation.is_federated(peer_domain)`.
  - `MtlsEnforcer::validate_peer_identity` (`lines 639-662`): validates peer identity against expected trust domain or federation manager.

#### 2.1.2 Identified Architectural Deficiencies
1. **No OIDC Token Authentication**: Workloads cannot authenticate using enterprise OIDC tokens (JWT Bearer tokens).
2. **Missing SPIRE Federation Mappings**: No structures mapping external OIDC claims (`iss`, `aud`, `sub`, roles) to allowed SPIFFE IDs or capability scopes.
3. **Privilege Escalation Risk**: External tokens must be constrained to explicit, narrow capability scopes without ambient authority.

#### 2.1.3 Interface Contract & Proposed Structures
```rust
// aura-core/src/identity/mod.rs

/// Enterprise OIDC identity provider registration according to SPIRE federation.
#[derive(Debug, Clone)]
pub struct OidcProviderConfig {
    pub issuer: String,
    pub audience: String,
    pub public_key_or_secret: Vec<u8>,
    pub default_scopes: Vec<String>,
}

/// SPIRE federation mapping rule translating external identity claims to local SVIDs.
#[derive(Debug, Clone)]
pub struct SpireFederationMapping {
    pub external_issuer: String,
    pub subject_pattern: String,
    pub mapped_spiffe_prefix: String,
    pub allowed_scopes: HashSet<String>,
}

impl FederationManager {
    /// Registers an external enterprise OIDC provider.
    pub fn register_oidc_provider(&self, provider: OidcProviderConfig);

    /// Registers a SPIRE federation claim-to-identity mapping.
    pub fn register_federation_mapping(&self, mapping: SpireFederationMapping);

    /// Authenticates an enterprise OIDC token (JWT) against registered federation mappings,
    /// returning a validated SVID with strictly constrained capabilities.
    pub fn verify_oidc_token(&self, token_jwt: &str) -> Result<Svid, IdentityError>;
}
```

---

### 2.2 Transport 1-RTT UDP Failure Detection & Silent HTTP/2 TCP Fallback (R4.2)

#### 2.2.1 Current Implementation & Code Citations
- **Source Files**: 
  - `aura-core/src/transport/http3.rs` (`lines 1-193`)
  - `aura-core/src/transport/mcp.rs` (`lines 1-635`)
- **Existing Functionality**:
  - `Http3Transport::start_with_config` (`lines 68-127`) binds:
    - UDP socket on `config.udp_bind_addr` for HTTP/3 QUIC traffic.
    - TCP listener on `config.tcp_bind_addr` for fallback traffic.
  - TCP fallback loop currently replies with static `HTTP/1.1 200 OK\r\nAlt-Svc: ...` and terminates connection without executing MCP requests (`lines 93-106`).
  - `McpServer` provides `handle_http_request(&self, http_raw: &str) -> String` (`mcp.rs:237-270`) which parses `GET /` (`tools/list`) and `POST /` (`tools/call`).

#### 2.2.2 Identified Architectural Deficiencies
1. **Server Does Not Execute MCP over TCP**: The TCP listener in `http3.rs` does not wire to `McpServer`. Requests falling back to TCP do not execute tools!
2. **Missing Client-Side 1-RTT Failure Detection**: There is no transport client that probes UDP/QUIC, detects packet drops or blocked UDP within 1 RTT (e.g. 50-100ms), and degrades gracefully.
3. **Risk of Dropped In-Flight Tool Calls**: If UDP drops packets mid-handshake or times out, pending tool calls must be seamlessly retried on TCP without surfacing errors to the caller.

#### 2.2.3 Interface Contract & Proposed Structures
```rust
// aura-core/src/transport/client.rs

/// Resilient MCP Transport Client with 1-RTT UDP failure detection and silent TCP failover.
pub struct ResilientMcpTransportClient {
    udp_target: SocketAddr,
    tcp_target: SocketAddr,
    rtt_timeout: Duration,
    cached_transport_mode: Arc<tokio::sync::RwLock<TransportMode>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportMode {
    Http3Quic,
    Http2TcpFallback,
}

impl ResilientMcpTransportClient {
    pub fn new(udp_target: SocketAddr, tcp_target: SocketAddr, rtt_timeout: Duration) -> Self;

    /// Dispatches a JSON-RPC MCP request.
    /// Attempts HTTP/3 QUIC first; if UDP fails within 1 RTT, seamlessly fails over to
    /// HTTP/2 over TCP without dropping in-flight tool calls.
    pub async fn call_tool(
        &self,
        request: JsonRpcRequest,
    ) -> Result<JsonRpcResponse, TransportError>;
}

// Server-side wiring in aura-core/src/transport/http3.rs
impl Http3Transport {
    /// Starts the HTTP/3 QUIC server and TCP fallback server with real McpServer execution.
    pub async fn start_mcp_listener_with_fallback(
        config: &TransportConfig,
        server: Arc<McpServer>,
    ) -> Result<Http3ServerHandle, Box<dyn std::error::Error>>;
}
```

---

## 3. File Boundaries and Module Impact

```
aura-core/
├── Cargo.toml                    # Verify ring / aes dependencies
├── src/
│   ├── lib.rs                    # Add `pub mod storage;`
│   ├── config/mod.rs             # Add StorageConfig to AuraConfig
│   ├── hypervisor/
│   │   ├── mod.rs                # Re-export EgressProxy types
│   │   ├── firecracker.rs        # Add VsockStreamConfig & read_stream_with_limit
│   │   └── proxy.rs              # Existing pure-Rust TLS proxy (ready)
│   ├── identity/
│   │   └── mod.rs                # Add OidcProviderConfig, SpireFederationMapping, verify_oidc_token
│   ├── storage/                  # NEW module
│   │   ├── mod.rs                # StorageError, module exports
│   │   ├── encrypted.rs          # EncryptedDatabase (AES-256-GCM + rusqlite)
│   │   └── ledger.rs             # TaskLedger persistent history
│   └── transport/
│       ├── mod.rs                # Export client & resilient types
│       ├── http3.rs              # Wire McpServer into TCP listener
│       ├── mcp.rs                # Tool execution engine (ready)
│       └── client.rs             # NEW: ResilientMcpTransportClient (1-RTT failover)
tests/
├── m5_microkernel_zero_trust_security_test.rs       # R3 validation test
└── m5_spire_federation_transport_fallback_test.rs   # R4 validation test
```

---

## 4. Verification Methods

### 4.1 Unit & Integration Tests
1. **Vsock Buffer Limit**:
   - `cargo test -p aura-core --lib hypervisor::firecracker::tests::test_vsock_stream_buffer_limit_and_truncation`
   - Test streams 2MB payload into a 32KB-limited listener; verifies host allocates <= 32KB, payload is truncated, no panics occur.
2. **Egress Proxy**:
   - `cargo test -p aura-core --lib hypervisor::proxy::tests` (7 existing tests all passing).
3. **Storage Encryption**:
   - `cargo test -p aura-core --lib storage::tests`
   - Test creates `EncryptedDatabase` and `TaskLedger`, writes task data, flushes to disk.
   - Inspects disk bytes: verifies magic header, verifies 0 occurrences of `"SQLite format 3"`, `"tasks"`, `"instruction"`.
   - Attempts decrypt with mismatched SVID: verifies authentication failure.
4. **SPIRE Federation & OIDC**:
   - `cargo test -p aura-core --lib identity::tests::test_spire_oidc_federation_mappings`
   - Test verifies valid OIDC token mints mapped SVID; expired/tampered tokens return `Err(IdentityError::...)`.
5. **Transport 1-RTT Fallback**:
   - `cargo test -p aura-core --test m5_spire_federation_transport_fallback_test`
   - Test sends `tools/call` with UDP blocked; asserts failover completes in < 100ms and in-flight tool call returns successful response.

### 4.2 Workspace Regressions Gate
- `cargo test -p aura-core --jobs 1` (146+ tests passing)
- `cargo test --manifest-path tools/ephemeral-tunnel/Cargo.toml` (6 tests passing)
- `cargo test --manifest-path deploy/images/vsock-screenshot-daemon/Cargo.toml` (1 test passing)
- `/home/jeff/aura/aura-sdk/.venv/bin/pytest aura-sdk/tests` (84 tests passing)
- `/home/jeff/aura/aura-sdk/.venv/bin/ruff check aura-sdk/aura_sdk aura-sdk/tests` (clean)
