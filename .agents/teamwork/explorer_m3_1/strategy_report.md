# Implementation Strategy Blueprint: Milestone 3 (Features 13 & 14)
**Target**: Aura Microkernel Zero-Trust Security Hardening  
**Components**: `aura-core/src/hypervisor/firecracker.rs`, `aura-core/src/hypervisor/proxy.rs`, `aura-core/src/hypervisor/mod.rs`  
**Author**: `explorer_m3_1`  
**Date**: 2026-10-11  

---

## 1. Executive Summary & Problem Boundary

Milestone 3 establishes microkernel Zero-Trust security isolation across all sandboxes and the host runtime. This report provides the concrete technical blueprint for implementing and wiring:
- **Feature 13 (Vsock Stream Buffer Limits)**: Preventing host memory exhaustion and denial-of-service from rogue or compromised microVM guest daemons through bounded stream buffers, instant stream truncation, and non-panicking error handling.
- **Feature 14 (Egress Proxy Module Re-Export & Wiring)**: Exposing the pure-Rust transparent TLS egress proxy (`EgressProxy`) at `aura-core::hypervisor`, wiring it directly into `FirecrackerVm` lifecycle management, and validating TLS Alert 49 (`0x15, 0x03, 0x03, 0x00, 0x02, 0x02, 0x31`, access denied) enforcement on unauthorized external requests.

Both features have been analyzed against existing unit tests, the 170-test opaque-box suite in `aura-core/tests/m5_e2e_opaque_test.rs`, and the requirements in `PROJECT.md` and `ORIGINAL_REQUEST.md`.

---

## 2. Feature 13: Vsock Stream Buffer Limits

### 2.1 Vulnerability in Current Codebase
In `aura-core/src/hypervisor/firecracker.rs` (lines 381-386):
```rust
tokio::spawn(async move {
    let mut buffer = Vec::new();
    if let Err(e) = stream.read_to_end(&mut buffer).await {
        eprintln!("Failed reading vsock screenshot stream: {}", e);
        return;
    }

    match process_and_save_screenshot(&buffer, &artifacts_dir).await {
...
```
`stream.read_to_end(&mut buffer)` executes an unbounded read. A rogue microVM guest daemon streaming tens of gigabytes of junk over virtio-vsock can force the host process to allocate memory until the Linux OOM-killer terminates the Aura kernel host or starves concurrent workloads.

### 2.2 Target Interface & Type Definitions
The implementer will add the following types and constants to `aura-core/src/hypervisor/firecracker.rs`:

```rust
/// Default maximum virtio-vsock stream buffer limit (16 MiB) to prevent host memory exhaustion (EXE-VM-015).
pub const DEFAULT_MAX_VSOCK_STREAM_BYTES: usize = 16 * 1024 * 1024;

/// Configuration for virtio-vsock stream buffer limits and host memory protection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct VsockStreamConfig {
    /// Maximum buffer size in bytes that the host will allocate when reading from an untrusted guest vsock stream.
    pub max_buffer_bytes: usize,
}

impl Default for VsockStreamConfig {
    fn default() -> Self {
        Self {
            max_buffer_bytes: DEFAULT_MAX_VSOCK_STREAM_BYTES,
        }
    }
}

impl VsockStreamConfig {
    pub fn new(max_buffer_bytes: usize) -> Self {
        Self { max_buffer_bytes }
    }
}

/// Typed error for virtio-vsock stream operations and memory bound enforcement.
#[derive(Debug)]
pub enum VsockStreamError {
    /// Underlying I/O error occurred during stream reading.
    Io(std::io::Error),
    /// Stream exceeded the configured maximum buffer limit and was truncated.
    BufferLimitExceeded { max_bytes: usize, bytes_read: usize },
    /// Other vsock stream or protocol error.
    Other(String),
}

impl std::fmt::Display for VsockStreamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VsockStreamError::Io(e) => write!(f, "Vsock stream I/O error: {}", e),
            VsockStreamError::BufferLimitExceeded { max_bytes, bytes_read } => {
                write!(
                    f,
                    "Vsock stream exceeded maximum buffer limit of {} bytes (truncated at {} bytes)",
                    max_bytes, bytes_read
                )
            }
            VsockStreamError::Other(msg) => write!(f, "Vsock stream error: {}", msg),
        }
    }
}

impl std::error::Error for VsockStreamError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            VsockStreamError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for VsockStreamError {
    fn from(err: std::io::Error) -> Self {
        VsockStreamError::Io(err)
    }
}
```

### 2.3 Instant Truncation Algorithm
The stream reader must:
1. Allocate dynamically as data arrives (never pre-allocate `max_bytes` upfront, preserving memory when small payloads arrive).
2. Read in chunks (e.g. 4096 bytes) bounded by `remaining + 1`.
3. If incoming bytes exceed `max_bytes`, copy only the allowed slice up to `max_bytes`, flag truncation, and immediately terminate reading.
4. Never read remaining rogue stream data into heap memory.

```rust
/// Reads an incoming asynchronous byte stream up to `max_bytes` with instant truncation.
///
/// Enforces Zero-Trust boundary (EXE-VM-015): untrusted guest microVMs cannot exhaust host
/// memory by streaming unbounded payloads. If incoming data exceeds `max_bytes`, the stream
/// is immediately truncated at exactly `max_bytes`, and reading terminates without allocating
/// additional memory or panicking.
///
/// Returns `Ok((buffer, truncated))` where `truncated` is true if the stream exceeded `max_bytes`.
pub async fn read_stream_with_limit_ext<R: AsyncRead + Unpin>(
    reader: &mut R,
    max_bytes: usize,
) -> Result<(Vec<u8>, bool), VsockStreamError> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 4096];
    let mut truncated = false;

    if max_bytes == 0 {
        // Probe whether stream has any incoming bytes without allocating heap
        let n = reader.read(&mut chunk[..1]).await?;
        if n > 0 {
            truncated = true;
        }
        return Ok((buffer, truncated));
    }

    loop {
        let remaining = max_bytes - buffer.len();
        if remaining == 0 {
            // Buffer is at capacity: probe if stream emits further bytes
            let n = reader.read(&mut chunk[..1]).await?;
            if n > 0 {
                truncated = true;
            }
            break;
        }

        let bytes_to_read = chunk.len().min(remaining + 1);
        let n = reader.read(&mut chunk[..bytes_to_read]).await?;
        if n == 0 {
            break; // EOF reached within limit
        }

        if buffer.len() + n > max_bytes {
            let allowed = max_bytes - buffer.len();
            buffer.extend_from_slice(&chunk[..allowed]);
            truncated = true;
            break; // Instantly truncate stream; terminate reading without memory leak
        } else {
            buffer.extend_from_slice(&chunk[..n]);
        }
    }

    Ok((buffer, truncated))
}

/// Reads an incoming asynchronous byte stream up to `max_bytes` with instant truncation.
///
/// Returns `Ok(Vec<u8>)` containing the received payload truncated to at most `max_bytes`.
pub async fn read_stream_with_limit<R: AsyncRead + Unpin>(
    reader: &mut R,
    max_bytes: usize,
) -> Result<Vec<u8>, VsockStreamError> {
    let (buf, _truncated) = read_stream_with_limit_ext(reader, max_bytes).await?;
    Ok(buf)
}
```

### 2.4 Refactoring `start_screenshot_listener`
Update `start_screenshot_listener` in `aura-core/src/hypervisor/firecracker.rs`:
```rust
/// Spawns an asynchronous vsock stream listener for screenshots with default buffer limit.
pub async fn start_screenshot_listener(
    socket_path: PathBuf,
    artifacts_dir: PathBuf,
) -> Result<ScreenshotListenerHandle, Box<dyn std::error::Error + Send + Sync>> {
    start_screenshot_listener_with_config(socket_path, artifacts_dir, VsockStreamConfig::default()).await
}

/// Spawns an asynchronous vsock stream listener for screenshots with explicit stream configuration.
pub async fn start_screenshot_listener_with_config(
    socket_path: PathBuf,
    artifacts_dir: PathBuf,
    config: VsockStreamConfig,
) -> Result<ScreenshotListenerHandle, Box<dyn std::error::Error + Send + Sync>> {
    tokio::fs::create_dir_all(&artifacts_dir).await?;

    if let Some(parent) = socket_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    if socket_path.exists() {
        let _ = tokio::fs::remove_file(&socket_path).await;
    }

    let listener = tokio::net::UnixListener::bind(&socket_path)?;
    println!(
        "Screenshot vsock listener active on {} (max_buffer_bytes: {})",
        socket_path.display(),
        config.max_buffer_bytes
    );

    let (shutdown_tx, mut shutdown_rx) = tokio::sync::broadcast::channel(1);
    let (tx, rx) = tokio::sync::mpsc::channel(32);

    let artifacts_dir_clone = artifacts_dir.clone();
    let socket_path_clone = socket_path.clone();

    let join_handle = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    println!("Screenshot vsock listener on {} shutting down", socket_path_clone.display());
                    break;
                }
                accept_res = listener.accept() => {
                    match accept_res {
                        Ok((mut stream, _addr)) => {
                            let artifacts_dir = artifacts_dir_clone.clone();
                            let tx = tx.clone();
                            tokio::spawn(async move {
                                let (buffer, truncated) = match read_stream_with_limit_ext(
                                    &mut stream,
                                    config.max_buffer_bytes,
                                ).await {
                                    Ok(res) => res,
                                    Err(e) => {
                                        eprintln!("Failed reading vsock screenshot stream: {}", e);
                                        return;
                                    }
                                };

                                if truncated {
                                    eprintln!(
                                        "Zero-Trust Warning: vsock stream truncated after reaching max buffer limit of {} bytes",
                                        config.max_buffer_bytes
                                    );
                                }

                                match process_and_save_screenshot(&buffer, &artifacts_dir).await {
                                    Ok(artifact) => {
                                        println!(
                                            "Successfully stored screenshot artifact: {:?} ({} bytes)",
                                            artifact.file_path, artifact.size_bytes
                                        );
                                        let _ = tx.send(artifact).await;
                                    }
                                    Err(e) => {
                                        eprintln!("Zero-Trust screenshot rejection: {}", e);
                                    }
                                }
                            });
                        }
                        Err(e) => {
                            eprintln!("Error accepting connection on vsock screenshot listener: {}", e);
                            break;
                        }
                    }
                }
            }
        }
        let _ = tokio::fs::remove_file(&socket_path_clone).await;
    });

    Ok(ScreenshotListenerHandle {
        socket_path,
        artifacts_dir,
        shutdown_tx,
        receiver: rx,
        join_handle,
    })
}
```

---

## 3. Feature 14: Egress Proxy Module Re-Export & Wiring

### 3.1 Verification of Existing `proxy.rs` Implementation
The investigation of `aura-core/src/hypervisor/proxy.rs` (lines 1-646) confirmed:
1. **RFC-Compliant Zero-FFI SNI Parsing** (`extract_sni`, lines 136-234):
   - Strict parsing of ContentType `0x16` (Handshake), Handshake Type `0x01` (ClientHello).
   - Bounds-checked navigation through Session ID, Cipher Suites, and Extension blocks.
   - Exact extraction of Extension `0x0000` (Server Name Indication) host name.
2. **Strict Domain Allowlist Enforcement** (`is_domain_allowed`, lines 107-132):
   - Supports exact matches (`crates.io`).
   - Supports subdomain matches (`static.crates.io`).
   - Supports wildcards (`*.ubuntu.com`).
   - Strictly rejects prefix collisions / domain spoofing (`evil-crates.io` is rejected).
3. **TLS Fatal Alert 49 Response** (`TLS_ALERT_ACCESS_DENIED`, lines 33-39 & 344-346):
   - `0x15`: TLS Record ContentType = Alert (21)
   - `0x03, 0x03`: ProtocolVersion = TLS 1.2 / TLS 1.3 record layer framing
   - `0x00, 0x02`: Record Length = 2 bytes
   - `0x02`: AlertLevel = Fatal (2)
   - `0x31`: AlertDescription = access_denied (49 = 0x31)
   - When unauthorized SNI or missing SNI is detected, `TLS_ALERT_ACCESS_DENIED` is immediately written to the client stream and the socket is shut down.
4. **HTTP CONNECT Handling** (lines 370-413):
   - Returns `HTTP/1.1 403 Forbidden` on unauthorized CONNECT requests.
   - Returns `HTTP/1.1 200 Connection Established` and tunnels allowed CONNECT targets.
5. **Default Deny**:
   - Any non-TLS / non-CONNECT egress traffic is rejected and shut down immediately.

### 3.2 Re-Exports in `aura-core/src/hypervisor/mod.rs`
Currently, `aura-core/src/hypervisor/mod.rs` declares `pub mod proxy;` but only re-exports items from `firecracker`.
Update `mod.rs` to re-export:

```rust
pub mod firecracker;
pub mod proxy;
pub mod wasm;

pub use firecracker::{
    format_redacted_handle, should_skip_kvm_validation, validate_kvm_at_path, validate_kvm_support,
    read_stream_with_limit, read_stream_with_limit_ext, BashCommandOutput, BashSessionHandle,
    BashSessionManager, CdpVsockForwarder, FirecrackerVm, KvmError, ScreenshotArtifact,
    ScreenshotListenerHandle, SpecializedRootfs, VmConfig, VsockStreamConfig, VsockStreamError,
    DEFAULT_BASH_VSOCK_PORT, DEFAULT_CDP_HOST_PORT, DEFAULT_CDP_VSOCK_PORT,
    DEFAULT_MAX_VSOCK_STREAM_BYTES, DEFAULT_SCREENSHOT_VSOCK_PORT,
};

pub use proxy::{
    EgressProxy, EgressProxyConfig, EgressProxyHandle, DEFAULT_ALLOWED_DOMAINS,
    TLS_ALERT_ACCESS_DENIED,
};
```

### 3.3 Wiring `EgressProxy` into `FirecrackerVm` Lifecycle
In `aura-core/src/hypervisor/firecracker.rs`:

1. Add fields to `FirecrackerVm`:
```rust
pub struct FirecrackerVm {
    pub id: String,
    pub svid: String,
    pub socket_path: PathBuf,
    pub chroot_dir: PathBuf,
    pub vsock_uds_path: PathBuf,
    jailer_child: Option<Child>,
    screenshot_socket: Option<PathBuf>,
    pub scratch_disk_path: Option<PathBuf>,
    pub cdp_forwarder: Option<CdpVsockForwarder>,
    pub egress_proxy: Option<crate::hypervisor::proxy::EgressProxyHandle>,
    pub vsock_stream_config: VsockStreamConfig,
}
```

2. Initialize in `FirecrackerVm::new` and `FirecrackerVm::with_config`:
```rust
Self {
    id: id.to_string(),
    svid: svid.to_string(),
    socket_path,
    chroot_dir,
    vsock_uds_path,
    jailer_child: None,
    screenshot_socket: None,
    scratch_disk_path: None,
    cdp_forwarder: None,
    egress_proxy: None,
    vsock_stream_config: VsockStreamConfig::default(),
}
```

3. Add proxy management methods to `FirecrackerVm`:
```rust
    /// Starts the transparent TLS egress proxy for this microVM (EXE-VM-015).
    pub async fn start_egress_proxy(
        &mut self,
        config: crate::hypervisor::proxy::EgressProxyConfig,
    ) -> Result<SocketAddr, Box<dyn std::error::Error + Send + Sync>> {
        let handle = crate::hypervisor::proxy::EgressProxy::start(config).await?;
        let bound_addr = handle.bind_addr;
        self.egress_proxy = Some(handle);
        Ok(bound_addr)
    }

    /// Stops the egress proxy if active.
    pub fn stop_egress_proxy(&mut self) {
        if let Some(proxy) = self.egress_proxy.take() {
            proxy.stop();
        }
    }
```

4. Wire into `FirecrackerVm::teardown`:
```rust
    // Stop TLS egress proxy if active (EXE-VM-015)
    self.stop_egress_proxy();
```
Because `Drop for FirecrackerVm` calls `self.teardown()`, the egress proxy is guaranteed to be shut down via RAII on microVM termination or error drops.

---

## 4. Concrete Verification Test Suite

The implementer should add the following targeted unit tests to `aura-core/src/hypervisor/firecracker.rs` (under `mod tests`):

```rust
    // =========================================================================
    // Feature 13 & 14 Unit Tests
    // =========================================================================

    #[tokio::test]
    async fn test_read_stream_under_limit_retained_completely() {
        let data = vec![0xAA; 1024];
        let mut cursor = std::io::Cursor::new(data.clone());
        let (buf, truncated) = read_stream_with_limit_ext(&mut cursor, 2048)
            .await
            .expect("read_stream_with_limit_ext failed");
        assert_eq!(buf, data);
        assert!(!truncated);
    }

    #[tokio::test]
    async fn test_read_stream_over_limit_instantly_truncated() {
        let data = vec![0xBB; 8192];
        let mut cursor = std::io::Cursor::new(data);
        let (buf, truncated) = read_stream_with_limit_ext(&mut cursor, 4096)
            .await
            .expect("read_stream_with_limit_ext failed");
        assert_eq!(buf.len(), 4096);
        assert!(truncated);
    }

    #[tokio::test]
    async fn test_read_stream_zero_byte_limit_truncates_immediately() {
        let data = vec![0xCC; 100];
        let mut cursor = std::io::Cursor::new(data);
        let (buf, truncated) = read_stream_with_limit_ext(&mut cursor, 0)
            .await
            .expect("read_stream_with_limit_ext failed");
        assert_eq!(buf.len(), 0);
        assert!(truncated);
    }

    #[tokio::test]
    async fn test_read_stream_empty_input_not_truncated() {
        let mut cursor = std::io::Cursor::new(Vec::new());
        let (buf, truncated) = read_stream_with_limit_ext(&mut cursor, 1024)
            .await
            .expect("read_stream_with_limit_ext failed");
        assert!(buf.is_empty());
        assert!(!truncated);
    }

    #[tokio::test]
    async fn test_read_stream_exact_boundary_not_truncated() {
        let data = vec![0xDD; 1000];
        let mut cursor = std::io::Cursor::new(data.clone());
        let (buf, truncated) = read_stream_with_limit_ext(&mut cursor, 1000)
            .await
            .expect("read_stream_with_limit_ext failed");
        assert_eq!(buf, data);
        assert!(!truncated);
    }

    #[tokio::test]
    async fn test_read_stream_huge_stream_bounded_memory() {
        let huge = vec![0xEE; 10 * 1024 * 1024]; // 10 MiB
        let mut cursor = std::io::Cursor::new(huge);
        let (buf, truncated) = read_stream_with_limit_ext(&mut cursor, 32 * 1024)
            .await
            .expect("read_stream_with_limit_ext failed");
        assert_eq!(buf.len(), 32 * 1024);
        assert!(truncated);
    }

    #[tokio::test]
    async fn test_screenshot_listener_with_custom_buffer_limit() {
        let temp_dir = tempfile::tempdir().unwrap();
        let sock_path = temp_dir.path().join("vsock_limited.sock");
        let artifacts_dir = temp_dir.path().join("artifacts");

        let config = VsockStreamConfig::new(64);
        let mut listener = start_screenshot_listener_with_config(
            sock_path.clone(),
            artifacts_dir.clone(),
            config,
        )
        .await
        .expect("Failed to start limited screenshot listener");

        // Send payload larger than 64 bytes
        let mut rogue_stream = Vec::from(PNG_MAGIC_BYTES);
        rogue_stream.extend_from_slice(&[0xFF; 256]);

        let mut client = TokioUnixStream::connect(&sock_path).await.unwrap();
        client.write_all(&rogue_stream).await.unwrap();
        client.shutdown().await.unwrap();

        // The listener truncates stream at 64 bytes, rejects incomplete PNG safely,
        // and does NOT panic or leak memory
        tokio::time::sleep(Duration::from_millis(50)).await;
        listener.stop();
    }

    #[tokio::test]
    async fn test_firecracker_vm_egress_proxy_lifecycle() {
        let mut vm = FirecrackerVm::new("test-vm-proxy", "spiffe://aura.local/test");
        assert!(vm.egress_proxy.is_none());

        let proxy_config = crate::hypervisor::proxy::EgressProxyConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            allowed_domains: vec!["crates.io".to_string()],
            enforce_strict_tls: true,
            upstream_override: None,
        };

        let proxy_addr = vm
            .start_egress_proxy(proxy_config)
            .await
            .expect("start_egress_proxy failed");
        assert!(vm.egress_proxy.is_some());

        // Connect client and send ClientHello targeting unauthorized domain
        let mut client = tokio::net::TcpStream::connect(proxy_addr).await.unwrap();
        let unauthorized_hello = crate::hypervisor::proxy::tests::create_test_client_hello("evil.com");
        client.write_all(&unauthorized_hello).await.unwrap();

        let mut response = Vec::new();
        client.read_to_end(&mut response).await.unwrap();
        assert_eq!(response, crate::hypervisor::proxy::TLS_ALERT_ACCESS_DENIED);

        vm.stop_egress_proxy();
        assert!(vm.egress_proxy.is_none());
    }
```

---

## 5. Verification Commands & Regression Checks

1. **Unit tests in `aura-core`**:
   ```bash
   cargo test -p aura-core --lib hypervisor
   ```
   Must pass all tests (expected: 44 tests passing).

2. **Full `aura-core` unit suite**:
   ```bash
   cargo test -p aura-core --lib
   ```
   Must pass all 135+ tests without regressions.

3. **Opaque E2E test suite**:
   ```bash
   cargo test -p aura-core --test m5_e2e_opaque_test
   ```
   Must pass all 170 tests cleanly.

4. **Formatting and check**:
   ```bash
   cargo check -p aura-core
   ```
   Must complete with zero errors and zero warnings.

---

## 6. Implementation Checklist for Worker

- [ ] Add `DEFAULT_MAX_VSOCK_STREAM_BYTES`, `VsockStreamConfig`, `VsockStreamError` to `aura-core/src/hypervisor/firecracker.rs`.
- [ ] Implement `read_stream_with_limit_ext` and `read_stream_with_limit` in `aura-core/src/hypervisor/firecracker.rs`.
- [ ] Implement `start_screenshot_listener_with_config` in `aura-core/src/hypervisor/firecracker.rs`.
- [ ] Refactor `start_screenshot_listener` to delegate to `start_screenshot_listener_with_config`.
- [ ] Add `egress_proxy: Option<EgressProxyHandle>` and `vsock_stream_config: VsockStreamConfig` to `FirecrackerVm`.
- [ ] Implement `start_egress_proxy`, `stop_egress_proxy`, and `start_screenshot_listener_with_config` on `FirecrackerVm`.
- [ ] Wire `self.stop_egress_proxy()` into `FirecrackerVm::teardown`.
- [ ] Update `aura-core/src/hypervisor/mod.rs` to re-export `EgressProxy`, `EgressProxyConfig`, `EgressProxyHandle`, `DEFAULT_ALLOWED_DOMAINS`, `TLS_ALERT_ACCESS_DENIED`, `read_stream_with_limit`, `read_stream_with_limit_ext`, `VsockStreamConfig`, `VsockStreamError`, and constants.
- [ ] Add unit tests to `firecracker.rs::tests`.
- [ ] Run verification commands and assert 100% test passage.
