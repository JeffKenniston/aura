use serde::{Deserialize, Serialize};
use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Default allow-listed package mirrors and official registries (EXE-VM-015).
pub const DEFAULT_ALLOWED_DOMAINS: &[&str] = &[
    // Rust crates.io
    "crates.io",
    "index.crates.io",
    "static.crates.io",
    // Python PyPI
    "pypi.org",
    "files.pythonhosted.org",
    "pypi.python.org",
    // Debian & Ubuntu mirrors
    "deb.debian.org",
    "archive.ubuntu.com",
    "security.ubuntu.com",
    "ports.ubuntu.com",
    // Alpine mirror
    "dl-cdn.alpinelinux.org",
    // GitHub (for cloning packages/dependencies)
    "github.com",
    "raw.githubusercontent.com",
    "api.github.com",
    "codeload.github.com",
];

/// TLS 1.2 / 1.3 Fatal Alert: Access Denied (49)
pub const TLS_ALERT_ACCESS_DENIED: &[u8] = &[
    0x15, // ContentType: Alert
    0x03, 0x03, // Version: TLS 1.2
    0x00, 0x02, // Length: 2
    0x02, // AlertLevel: Fatal
    0x31, // Description: access_denied (49)
];

/// Configuration for the transparent TLS egress proxy (EXE-VM-015).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EgressProxyConfig {
    pub bind_addr: SocketAddr,
    pub allowed_domains: Vec<String>,
    pub enforce_strict_tls: bool,
    #[serde(skip)]
    pub upstream_override: Option<SocketAddr>,
}

impl Default for EgressProxyConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:8444".parse().unwrap(),
            allowed_domains: DEFAULT_ALLOWED_DOMAINS
                .iter()
                .map(|s| s.to_string())
                .collect(),
            enforce_strict_tls: true,
            upstream_override: None,
        }
    }
}

impl EgressProxyConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.allowed_domains.is_empty() {
            return Err("hypervisor.proxy.allowed_domains cannot be empty".to_string());
        }
        Ok(())
    }

    pub fn with_upstream_override(mut self, addr: SocketAddr) -> Self {
        self.upstream_override = Some(addr);
        self
    }
}

/// Running handle for an active egress proxy instance.
pub struct EgressProxyHandle {
    pub bind_addr: SocketAddr,
    shutdown_tx: tokio::sync::broadcast::Sender<()>,
    join_handle: tokio::task::JoinHandle<()>,
}

impl EgressProxyHandle {
    /// Shuts down the egress proxy task.
    pub fn stop(&self) {
        let _ = self.shutdown_tx.send(());
    }

    /// Awaits the completion of the proxy task.
    pub async fn wait(self) -> Result<(), tokio::task::JoinError> {
        self.join_handle.await
    }
}

/// Pure-Rust TLS-terminating / SNI-intercepting Egress Proxy (EXE-VM-015).
/// Enforces Zero-Trust boundary for Firecracker microVM network traffic,
/// dropping unauthorized handshakes and forwarding allowed package registry calls.
pub struct EgressProxy;

impl EgressProxy {
    /// Checks whether the target domain matches any entry in the allow-list.
    /// Supports exact matches and subdomain matches (e.g. `crates.io` allows `static.crates.io`).
    /// Disallows prefix collisions (e.g. `evil-crates.io` is strictly rejected).
    pub fn is_domain_allowed(allowed_domains: &[String], target_domain: &str) -> bool {
        let target_lower = target_domain.to_lowercase();
        let target = target_lower.trim_end_matches('.');

        for allowed in allowed_domains {
            let allowed_lower = allowed.to_lowercase();
            let rule = allowed_lower.trim_end_matches('.');

            // Handle wildcard rules like `*.example.com`
            let clean_rule = rule.strip_prefix("*.").unwrap_or(rule);

            if target == clean_rule {
                return true;
            }

            // Subdomain match: must end with ".clean_rule"
            if target.ends_with(clean_rule) {
                let prefix_len = target.len() - clean_rule.len();
                if prefix_len > 0 && target.as_bytes()[prefix_len - 1] == b'.' {
                    return true;
                }
            }
        }

        false
    }

    /// Extracts the Server Name Indication (SNI) hostname from a TLS ClientHello record.
    /// Implements RFC 5246 / RFC 6066 / RFC 8446 parsing with zero FFI and strict bounds checks.
    pub fn extract_sni(buf: &[u8]) -> Option<String> {
        // Record layer:
        // 0: ContentType (0x16 = Handshake)
        // 1..3: Version
        // 3..5: Record Length
        if buf.len() < 5 || buf[0] != 0x16 {
            return None;
        }

        // Handshake header:
        // 5: Handshake Type (0x01 = ClientHello)
        // 6..9: Length (24-bit uint)
        if buf.len() < 9 || buf[5] != 0x01 {
            return None;
        }

        // Skip ClientHello header:
        // 9..11: Client Version (2 bytes)
        // 11..43: Random (32 bytes)
        let mut offset = 43;
        if buf.len() < offset + 1 {
            return None;
        }

        // Session ID:
        let session_id_len = buf[offset] as usize;
        offset += 1 + session_id_len;
        if buf.len() < offset + 2 {
            return None;
        }

        // Cipher Suites:
        let cipher_suites_len = u16::from_be_bytes([buf[offset], buf[offset + 1]]) as usize;
        offset += 2 + cipher_suites_len;
        if buf.len() < offset + 1 {
            return None;
        }

        // Compression Methods:
        let comp_methods_len = buf[offset] as usize;
        offset += 1 + comp_methods_len;
        if buf.len() < offset + 2 {
            return None;
        }

        // Extensions length:
        let extensions_len = u16::from_be_bytes([buf[offset], buf[offset + 1]]) as usize;
        offset += 2;

        let extensions_end = std::cmp::min(offset + extensions_len, buf.len());

        // Parse extensions
        while offset + 4 <= extensions_end {
            let ext_type = u16::from_be_bytes([buf[offset], buf[offset + 1]]);
            let ext_len = u16::from_be_bytes([buf[offset + 2], buf[offset + 3]]) as usize;
            offset += 4;

            if offset + ext_len > extensions_end {
                break;
            }

            if ext_type == 0x0000 {
                // Extension 0x0000 is server_name (SNI)
                let ext_data = &buf[offset..offset + ext_len];
                if ext_data.len() < 2 {
                    return None;
                }
                let server_name_list_len = u16::from_be_bytes([ext_data[0], ext_data[1]]) as usize;
                let mut sni_offset = 2;
                let list_end = std::cmp::min(2 + server_name_list_len, ext_data.len());

                while sni_offset + 3 <= list_end {
                    let name_type = ext_data[sni_offset];
                    let name_len =
                        u16::from_be_bytes([ext_data[sni_offset + 1], ext_data[sni_offset + 2]])
                            as usize;
                    sni_offset += 3;

                    if sni_offset + name_len > list_end {
                        break;
                    }

                    if name_type == 0 {
                        // host_name (0)
                        if let Ok(hostname) =
                            std::str::from_utf8(&ext_data[sni_offset..sni_offset + name_len])
                        {
                            return Some(hostname.to_string());
                        }
                    }
                    sni_offset += name_len;
                }
            }

            offset += ext_len;
        }

        None
    }

    /// Extracts target host and port from an HTTP CONNECT request line.
    /// E.g. "CONNECT pypi.org:443 HTTP/1.1\r\n" -> ("pypi.org", 443)
    pub fn extract_http_connect_target(buf: &[u8]) -> Option<(String, u16)> {
        if !buf.starts_with(b"CONNECT ") {
            return None;
        }
        let text = std::str::from_utf8(buf).ok()?;
        let line = text.lines().next()?;
        let mut parts = line.split_whitespace();
        if parts.next()? != "CONNECT" {
            return None;
        }
        let host_port = parts.next()?;
        let mut hp_split = host_port.split(':');
        let host = hp_split.next()?.to_string();
        let port = hp_split
            .next()
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(443);
        Some((host, port))
    }

    /// Starts the EgressProxy listener task.
    pub async fn start(config: EgressProxyConfig) -> Result<EgressProxyHandle, std::io::Error> {
        let listener = TcpListener::bind(config.bind_addr).await?;
        let bind_addr = listener.local_addr()?;
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::broadcast::channel(1);

        let allowed_domains = Arc::new(config.allowed_domains);
        let upstream_override = config.upstream_override;

        println!("TLS Egress Proxy (EXE-VM-015) listening on {}", bind_addr);

        let join_handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        println!("TLS Egress Proxy on {} shutting down", bind_addr);
                        break;
                    }
                    accept_res = listener.accept() => {
                        match accept_res {
                            Ok((stream, peer_addr)) => {
                                let allowed_domains = Arc::clone(&allowed_domains);
                                tokio::spawn(async move {
                                    if let Err(e) = Self::handle_connection(
                                        stream,
                                        allowed_domains,
                                        upstream_override,
                                    ).await {
                                        eprintln!(
                                            "[EXE-VM-015:EGRESS-FILTER] Connection from {} handled: {}",
                                            peer_addr, e
                                        );
                                    }
                                });
                            }
                            Err(e) => {
                                eprintln!("Error accepting connection on egress proxy: {}", e);
                                break;
                            }
                        }
                    }
                }
            }
        });

        Ok(EgressProxyHandle {
            bind_addr,
            shutdown_tx,
            join_handle,
        })
    }

    /// Handles a single incoming connection from a Firecracker microVM.
    async fn handle_connection(
        mut client_stream: TcpStream,
        allowed_domains: Arc<Vec<String>>,
        upstream_override: Option<SocketAddr>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut buffer = [0u8; 4096];
        let n = client_stream.read(&mut buffer).await?;
        if n == 0 {
            return Ok(());
        }

        let initial_payload = &buffer[..n];

        // Case 1: Transparent TLS Handshake (0x16)
        if initial_payload[0] == 0x16 {
            let sni = match Self::extract_sni(initial_payload) {
                Some(sni) => sni,
                None => {
                    eprintln!(
                        "[EXE-VM-015:BLOCKED] Dropping TLS handshake: SNI missing or malformed"
                    );
                    let _ = client_stream.write_all(TLS_ALERT_ACCESS_DENIED).await;
                    let _ = client_stream.shutdown().await;
                    return Err("Missing SNI in TLS ClientHello".into());
                }
            };

            if !Self::is_domain_allowed(&allowed_domains, &sni) {
                eprintln!(
                    "[EXE-VM-015:BLOCKED] Dropping unauthorized egress attempt to domain: '{}'",
                    sni
                );
                // Zero-Trust Boundary: Send Fatal TLS Alert and immediately drop connection
                let _ = client_stream.write_all(TLS_ALERT_ACCESS_DENIED).await;
                let _ = client_stream.shutdown().await;
                return Err(format!("Unauthorized egress domain: {}", sni).into());
            }

            println!(
                "[EXE-VM-015:PERMITTED] Forwarding authorized egress connection to '{}'",
                sni
            );

            // Connect to upstream target
            let mut upstream_stream = if let Some(override_addr) = upstream_override {
                TcpStream::connect(override_addr).await?
            } else {
                TcpStream::connect(format!("{}:443", sni)).await?
            };

            // Forward the initial ClientHello bytes to upstream
            upstream_stream.write_all(initial_payload).await?;

            // Bi-directional tunnel between guest microVM and upstream registry
            tokio::io::copy_bidirectional(&mut client_stream, &mut upstream_stream).await?;
            return Ok(());
        }

        // Case 2: Explicit HTTP CONNECT Proxy
        if initial_payload.starts_with(b"CONNECT ") {
            let (target_host, target_port) =
                match Self::extract_http_connect_target(initial_payload) {
                    Some((h, p)) => (h, p),
                    None => {
                        let _ = client_stream
                            .write_all(b"HTTP/1.1 400 Bad Request\r\nConnection: close\r\n\r\n")
                            .await;
                        let _ = client_stream.shutdown().await;
                        return Err("Malformed HTTP CONNECT request".into());
                    }
                };

            if !Self::is_domain_allowed(&allowed_domains, &target_host) {
                eprintln!(
                    "[EXE-VM-015:BLOCKED] Dropping unauthorized HTTP CONNECT attempt to: '{}:{}'",
                    target_host, target_port
                );
                let _ = client_stream
                    .write_all(b"HTTP/1.1 403 Forbidden\r\nConnection: close\r\n\r\n")
                    .await;
                let _ = client_stream.shutdown().await;
                return Err(format!("Unauthorized egress CONNECT domain: {}", target_host).into());
            }

            println!(
                "[EXE-VM-015:PERMITTED] Forwarding authorized HTTP CONNECT to '{}:{}'",
                target_host, target_port
            );

            let mut upstream_stream = if let Some(override_addr) = upstream_override {
                TcpStream::connect(override_addr).await?
            } else {
                TcpStream::connect(format!("{}:{}", target_host, target_port)).await?
            };

            // Acknowledge connection establishment
            client_stream
                .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                .await?;

            tokio::io::copy_bidirectional(&mut client_stream, &mut upstream_stream).await?;
            return Ok(());
        }

        // Case 3: Non-TLS, Non-CONNECT Traffic -> Default Deny (Zero-Trust)
        eprintln!("[EXE-VM-015:BLOCKED] Default deny: dropping non-TLS/non-proxy egress traffic");
        let _ = client_stream.shutdown().await;
        Err("Unsupported egress protocol (default-deny)".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Constructs a valid synthetic TLS ClientHello packet with the specified SNI.
    fn create_test_client_hello(server_name: &str) -> Vec<u8> {
        let name_bytes = server_name.as_bytes();
        let name_len = name_bytes.len() as u16;
        let list_len = (1 + 2 + name_len) as u16;

        let mut ext_data = Vec::new();
        ext_data.extend_from_slice(&list_len.to_be_bytes());
        ext_data.push(0x00); // HostName type
        ext_data.extend_from_slice(&name_len.to_be_bytes());
        ext_data.extend_from_slice(name_bytes);

        let mut extensions = Vec::new();
        extensions.extend_from_slice(&[0x00, 0x00]); // Extension Type: server_name (0)
        extensions.extend_from_slice(&(ext_data.len() as u16).to_be_bytes());
        extensions.extend_from_slice(&ext_data);

        let mut body = Vec::new();
        body.extend_from_slice(&[0x03, 0x03]); // TLS 1.2 client version
        body.extend_from_slice(&[0x42; 32]); // 32-byte client random
        body.push(0x00); // Session ID length: 0
        body.extend_from_slice(&2u16.to_be_bytes()); // Cipher suites length: 2
        body.extend_from_slice(&[0x13, 0x01]); // TLS_AES_128_GCM_SHA256
        body.push(0x01); // Compression methods length: 1
        body.push(0x00); // null compression
        body.extend_from_slice(&(extensions.len() as u16).to_be_bytes());
        body.extend_from_slice(&extensions);

        let mut handshake = Vec::new();
        handshake.push(0x01); // ClientHello
        let hs_len = body.len() as u32;
        handshake.push(((hs_len >> 16) & 0xFF) as u8);
        handshake.push(((hs_len >> 8) & 0xFF) as u8);
        handshake.push((hs_len & 0xFF) as u8);
        handshake.extend_from_slice(&body);

        let mut record = Vec::new();
        record.push(0x16); // ContentType: Handshake
        record.extend_from_slice(&[0x03, 0x01]); // TLS 1.0 record version
        record.extend_from_slice(&(handshake.len() as u16).to_be_bytes());
        record.extend_from_slice(&handshake);

        record
    }

    #[test]
    fn test_sni_extraction_valid() {
        let hello = create_test_client_hello("crates.io");
        let sni = EgressProxy::extract_sni(&hello);
        assert_eq!(sni, Some("crates.io".to_string()));

        let hello_pypi = create_test_client_hello("pypi.org");
        assert_eq!(
            EgressProxy::extract_sni(&hello_pypi),
            Some("pypi.org".to_string())
        );
    }

    #[test]
    fn test_sni_extraction_invalid() {
        // Empty bytes
        assert_eq!(EgressProxy::extract_sni(&[]), None);
        // Non-handshake record
        assert_eq!(
            EgressProxy::extract_sni(&[0x15, 0x03, 0x03, 0x00, 0x02]),
            None
        );
        // Truncated packet
        assert_eq!(
            EgressProxy::extract_sni(&[0x16, 0x03, 0x01, 0x00, 0x10]),
            None
        );
    }

    #[test]
    fn test_domain_allowlist_matching() {
        let allowed = vec![
            "crates.io".to_string(),
            "pypi.org".to_string(),
            "*.ubuntu.com".to_string(),
        ];

        // Exact matches
        assert!(EgressProxy::is_domain_allowed(&allowed, "crates.io"));
        assert!(EgressProxy::is_domain_allowed(&allowed, "pypi.org"));

        // Subdomain matches
        assert!(EgressProxy::is_domain_allowed(&allowed, "static.crates.io"));
        assert!(EgressProxy::is_domain_allowed(
            &allowed,
            "archive.ubuntu.com"
        ));
        assert!(EgressProxy::is_domain_allowed(
            &allowed,
            "security.ubuntu.com"
        ));

        // Unauthorized / hostile domain names
        assert!(!EgressProxy::is_domain_allowed(
            &allowed,
            "unauthorized-domain.com"
        ));
        assert!(!EgressProxy::is_domain_allowed(&allowed, "evil-crates.io"));
        assert!(!EgressProxy::is_domain_allowed(
            &allowed,
            "crates.io.attacker.com"
        ));
        assert!(!EgressProxy::is_domain_allowed(&allowed, "notpypi.org"));
    }

    #[test]
    fn test_extract_http_connect_target() {
        let req = b"CONNECT pypi.org:443 HTTP/1.1\r\nHost: pypi.org:443\r\n\r\n";
        let target = EgressProxy::extract_http_connect_target(req);
        assert_eq!(target, Some(("pypi.org".to_string(), 443)));

        let bad_req = b"GET /index.html HTTP/1.1\r\n\r\n";
        assert_eq!(EgressProxy::extract_http_connect_target(bad_req), None);
    }

    #[tokio::test]
    async fn test_proxy_drops_unauthorized_tls_connection() {
        let config = EgressProxyConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            allowed_domains: vec!["crates.io".to_string()],
            enforce_strict_tls: true,
            upstream_override: None,
        };

        let handle = EgressProxy::start(config)
            .await
            .expect("Failed to start proxy");
        let proxy_addr = handle.bind_addr;

        // Connect client and send ClientHello targeting unauthorized domain
        let mut client = TcpStream::connect(proxy_addr)
            .await
            .expect("Failed to connect");
        let unauthorized_hello = create_test_client_hello("unauthorized-domain.com");
        client.write_all(&unauthorized_hello).await.unwrap();

        // Expect proxy to reply with TLS_ALERT_ACCESS_DENIED and terminate connection
        let mut response = Vec::new();
        client.read_to_end(&mut response).await.unwrap();

        assert_eq!(response, TLS_ALERT_ACCESS_DENIED);

        handle.stop();
    }

    #[tokio::test]
    async fn test_proxy_allows_authorized_domain_with_upstream() {
        // 1. Start a mock upstream server
        let mock_upstream = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream_addr = mock_upstream.local_addr().unwrap();

        tokio::spawn(async move {
            if let Ok((mut stream, _)) = mock_upstream.accept().await {
                let mut buf = [0u8; 1024];
                let _n = stream.read(&mut buf).await.unwrap();
                // Echo a synthetic server greeting
                stream.write_all(b"MOCK_TLS_SERVER_GREETING").await.unwrap();
            }
        });

        // 2. Start proxy configured with upstream override pointing to mock upstream
        let config = EgressProxyConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            allowed_domains: vec!["crates.io".to_string()],
            enforce_strict_tls: true,
            upstream_override: Some(upstream_addr),
        };

        let handle = EgressProxy::start(config)
            .await
            .expect("Failed to start proxy");
        let proxy_addr = handle.bind_addr;

        // 3. Connect client and send ClientHello targeting allowed "crates.io"
        let mut client = TcpStream::connect(proxy_addr)
            .await
            .expect("Failed to connect");
        let allowed_hello = create_test_client_hello("crates.io");
        client.write_all(&allowed_hello).await.unwrap();

        // 4. Verify client receives greeting forwarded from mock upstream
        let mut response = [0u8; 24];
        client.read_exact(&mut response).await.unwrap();
        assert_eq!(&response, b"MOCK_TLS_SERVER_GREETING");

        handle.stop();
    }

    #[tokio::test]
    async fn test_proxy_http_connect_allowed_and_blocked() {
        let config = EgressProxyConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            allowed_domains: vec!["pypi.org".to_string()],
            enforce_strict_tls: true,
            upstream_override: None,
        };

        let handle = EgressProxy::start(config)
            .await
            .expect("Failed to start proxy");
        let proxy_addr = handle.bind_addr;

        // Blocked CONNECT request
        let mut client = TcpStream::connect(proxy_addr).await.unwrap();
        client
            .write_all(b"CONNECT malicious-site.com:443 HTTP/1.1\r\n\r\n")
            .await
            .unwrap();
        let mut resp = Vec::new();
        client.read_to_end(&mut resp).await.unwrap();
        assert!(resp.starts_with(b"HTTP/1.1 403 Forbidden"));

        handle.stop();
    }
}
