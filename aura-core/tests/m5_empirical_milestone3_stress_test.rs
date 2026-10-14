//! Adversarial Empirical Stress Test Suite for Milestone 3 (Features 13-17)
//!
//! Target Verification Areas:
//! 1. Feature 14: Transparent TLS EgressProxy wiring and TLS Alert 49
//!    (0x15, 0x03, 0x03, 0x00, 0x02, 0x02, 0x31) on unauthorized domain SNIs vs passing authorized domains.
//!    Verify RAII lifecycle cleanup on teardown and Drop for FirecrackerVm.
//! 2. Features 15-17: EncryptedDatabase & TaskLedger. Stress test in-memory SQLite operations,
//!    serialization/deserialization cycles, and verify that disk files remain strictly encrypted with AES-256-GCM.
//! 3. Feature 13: Vsock stream memory buffer limits and instant truncation without panic.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;

use aura_core::hypervisor::firecracker::{
    read_stream_with_limit_ext, FirecrackerVm, VsockStreamConfig,
};
use aura_core::hypervisor::proxy::{
    EgressProxy, EgressProxyConfig, TLS_ALERT_ACCESS_DENIED,
};
use aura_core::identity::Svid;
use aura_core::storage::{
    derive_storage_key, CryptoEnvelope, EncryptedDatabase, StorageError, TaskEvent,
    TaskLedger, TaskState, ENCRYPTED_MAGIC, MIN_ENVELOPE_LEN, NONCE_LEN,
};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Expected TLS 1.2 / 1.3 Fatal Alert 49: Access Denied
const EXPECTED_TLS_ALERT_49: [u8; 7] = [0x15, 0x03, 0x03, 0x00, 0x02, 0x02, 0x31];

/// Helper constructing a valid synthetic TLS ClientHello with the given SNI hostname.
fn build_tls_client_hello(sni_hostname: &str) -> Vec<u8> {
    let name_bytes = sni_hostname.as_bytes();
    let name_len = name_bytes.len() as u16;
    let list_len = (1 + 2 + name_len) as u16;
    let ext_data_len = (2 + list_len) as u16;

    let mut ext = Vec::new();
    ext.extend_from_slice(&0x0000u16.to_be_bytes()); // extension type: server_name (0)
    ext.extend_from_slice(&ext_data_len.to_be_bytes());
    ext.extend_from_slice(&list_len.to_be_bytes());
    ext.push(0x00); // host_name type
    ext.extend_from_slice(&name_len.to_be_bytes());
    ext.extend_from_slice(name_bytes);

    let mut handshake = Vec::new();
    handshake.extend_from_slice(&0x0303u16.to_be_bytes()); // TLS 1.2 client version
    handshake.extend_from_slice(&[0x42; 32]); // client random
    handshake.push(0x00); // session id length
    handshake.extend_from_slice(&0x0002u16.to_be_bytes()); // cipher suites length
    handshake.extend_from_slice(&0xc02fu16.to_be_bytes()); // TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256
    handshake.extend_from_slice(&[0x01, 0x00]); // compression methods
    handshake.extend_from_slice(&(ext.len() as u16).to_be_bytes()); // extensions length
    handshake.extend_from_slice(&ext);

    let mut record = Vec::new();
    record.push(0x16); // Handshake record
    record.extend_from_slice(&0x0301u16.to_be_bytes()); // Record layer version: TLS 1.0

    let hs_len = (handshake.len() + 4) as u32;
    record.extend_from_slice(&(hs_len as u16).to_be_bytes());

    record.push(0x01); // ClientHello handshake type
    record.push((handshake.len() >> 16) as u8);
    record.push((handshake.len() >> 8) as u8);
    record.push(handshake.len() as u8);
    record.extend_from_slice(&handshake);

    record
}

/// Helper calculating Shannon Entropy of byte slice.
fn shannon_entropy(bytes: &[u8]) -> f64 {
    if bytes.is_empty() {
        return 0.0;
    }
    let mut counts = [0usize; 256];
    for &b in bytes {
        counts[b as usize] += 1;
    }
    let total = bytes.len() as f64;
    let mut entropy = 0.0;
    for &count in &counts {
        if count > 0 {
            let p = count as f64 / total;
            entropy -= p * p.log2();
        }
    }
    entropy
}

// ============================================================================
// AREA 1: Feature 14 - TLS EgressProxy & Alert 49 & Lifecycle Cleanup
// ============================================================================

#[tokio::test]
async fn test_empirical_egress_proxy_alert_49_on_unauthorized_domains() {
    assert_eq!(
        TLS_ALERT_ACCESS_DENIED, EXPECTED_TLS_ALERT_49,
        "Constant TLS_ALERT_ACCESS_DENIED must match exact RFC Alert 49 bytes"
    );

    let config = EgressProxyConfig {
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        allowed_domains: vec![
            "crates.io".to_string(),
            "pypi.org".to_string(),
            "github.com".to_string(),
        ],
        enforce_strict_tls: true,
        upstream_override: None,
    };

    let handle = EgressProxy::start(config)
        .await
        .expect("Failed to start EgressProxy");
    let proxy_addr = handle.bind_addr;

    let unauthorized_domains = [
        "evil.com",
        "attacker.org",
        "crates.io.attacker.org",
        "notcrates.io",
        "pypi.org.evil.net",
        "github.com.phishing.io",
        "subdomain.blocked.net",
        "192.168.1.1",
    ];

    for domain in unauthorized_domains {
        let mut stream = TcpStream::connect(proxy_addr)
            .await
            .expect("Failed to connect to proxy");

        let client_hello = build_tls_client_hello(domain);
        stream
            .write_all(&client_hello)
            .await
            .expect("Failed to write ClientHello");

        let mut response = Vec::new();
        stream
            .read_to_end(&mut response)
            .await
            .expect("Failed to read proxy response");

        assert_eq!(
            response, EXPECTED_TLS_ALERT_49,
            "Proxy must emit exact TLS Alert 49 on unauthorized domain '{}'",
            domain
        );
    }

    handle.stop();
}

#[tokio::test]
async fn test_empirical_egress_proxy_forwards_authorized_domains_to_upstream() {
    // 1. Spawn a mock upstream TLS server on loopback
    let upstream_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind mock upstream listener");
    let upstream_addr = upstream_listener.local_addr().unwrap();

    let upstream_task = tokio::spawn(async move {
        let (mut socket, _) = upstream_listener
            .accept()
            .await
            .expect("Mock upstream accept failed");

        let mut buf = [0u8; 1024];
        let n = socket
            .read(&mut buf)
            .await
            .expect("Mock upstream read failed");
        assert!(n > 0, "Upstream must receive forwarded ClientHello bytes");

        // Send mock TLS ServerHello response
        let mock_server_hello = b"\x16\x03\x03\x00\x08SERVEROK";
        socket
            .write_all(mock_server_hello)
            .await
            .expect("Mock upstream write failed");
    });

    // 2. Start proxy with upstream_override pointed at mock server
    let config = EgressProxyConfig {
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        allowed_domains: vec!["crates.io".to_string(), "static.crates.io".to_string()],
        enforce_strict_tls: true,
        upstream_override: Some(upstream_addr),
    };

    let handle = EgressProxy::start(config)
        .await
        .expect("Failed to start proxy");
    let proxy_addr = handle.bind_addr;

    // 3. Connect client and send ClientHello for authorized domain "crates.io"
    let mut client = TcpStream::connect(proxy_addr)
        .await
        .expect("Client connect to proxy failed");

    let hello = build_tls_client_hello("crates.io");
    client
        .write_all(&hello)
        .await
        .expect("Client write hello failed");

    let mut response = [0u8; 13];
    client
        .read_exact(&mut response)
        .await
        .expect("Client read response failed");

    assert_eq!(
        &response, b"\x16\x03\x03\x00\x08SERVEROK",
        "Authorized domain must be cleanly forwarded to upstream without TLS Alert 49"
    );

    upstream_task.await.unwrap();
    handle.stop();
}

#[tokio::test]
async fn test_empirical_firecracker_vm_egress_proxy_raii_teardown_cleanup() {
    let mut vm = FirecrackerVm::new("test-vm-teardown", "spiffe://aura.local/agent/workload");
    assert!(vm.egress_proxy.is_none());

    let config = EgressProxyConfig {
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        allowed_domains: vec!["crates.io".to_string()],
        enforce_strict_tls: true,
        upstream_override: None,
    };

    let proxy_addr = vm
        .start_egress_proxy(config)
        .await
        .expect("start_egress_proxy failed");
    assert!(vm.egress_proxy.is_some());

    // Verify proxy port is actively listening and responsive
    let mut stream = TcpStream::connect(proxy_addr)
        .await
        .expect("Proxy port must be open");
    let unauthorized_hello = build_tls_client_hello("bad.com");
    stream.write_all(&unauthorized_hello).await.unwrap();
    let mut resp = Vec::new();
    stream.read_to_end(&mut resp).await.unwrap();
    assert_eq!(resp, EXPECTED_TLS_ALERT_49);

    // Call teardown(): must stop proxy and clear handle
    vm.teardown().expect("teardown failed");
    assert!(vm.egress_proxy.is_none());

    // Give proxy loop moment to shut down
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Verify proxy port is closed (connection refused)
    let connect_res = TcpStream::connect(proxy_addr).await;
    assert!(
        connect_res.is_err(),
        "Proxy port must be closed after teardown()"
    );
}

#[tokio::test]
async fn test_empirical_firecracker_vm_drop_guard_raii_cleanup() {
    let proxy_addr;

    {
        let mut vm = FirecrackerVm::new("test-vm-drop", "spiffe://aura.local/agent/drop");
        let config = EgressProxyConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            allowed_domains: vec!["crates.io".to_string()],
            enforce_strict_tls: true,
            upstream_override: None,
        };

        proxy_addr = vm
            .start_egress_proxy(config)
            .await
            .expect("start_egress_proxy failed");

        // Actively verify port is listening
        let stream = TcpStream::connect(proxy_addr).await;
        assert!(stream.is_ok(), "Proxy port must be open inside scope");

        // End of scope: vm is dropped here, invoking Drop -> teardown() -> stop_egress_proxy()
    }

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Verify proxy port is closed automatically by Drop
    let connect_res = TcpStream::connect(proxy_addr).await;
    assert!(
        connect_res.is_err(),
        "Proxy port must be closed after FirecrackerVm is dropped"
    );
}

// ============================================================================
// AREA 2: Features 15-17 - EncryptedDatabase & TaskLedger Stress & Security
// ============================================================================

#[test]
fn test_empirical_encrypted_database_100_generation_persistence_cycle() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("lifecycle_100.db");
    let svid = Svid::new("spiffe://aura.local/agent/generation-stress");
    let key = derive_storage_key(&svid);

    // Perform 100 sequential open -> mutate -> flush -> drop cycles
    for gen in 1..=100 {
        let db = EncryptedDatabase::open(&db_path, key)
            .unwrap_or_else(|e| panic!("Failed to open DB at generation {}: {:?}", gen, e));

        if gen == 1 {
            db.execute_batch(
                "CREATE TABLE generations (
                    gen_id INTEGER PRIMARY KEY,
                    tag TEXT NOT NULL,
                    created_at INTEGER NOT NULL
                );",
            )
            .unwrap();
        }

        db.execute(
            "INSERT INTO generations (gen_id, tag, created_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![gen, format!("gen-tag-{}", gen), 1000 + gen],
        )
        .unwrap();

        db.flush()
            .unwrap_or_else(|e| panic!("Failed to flush at generation {}: {:?}", gen, e));
    }

    // Reopen and assert all 100 records survived with complete fidelity
    let db = EncryptedDatabase::open(&db_path, key).unwrap();
    let count: i64 = db
        .query_row("SELECT COUNT(*) FROM generations", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 100, "All 100 generations must persist");

    for gen in 1..=100 {
        let tag: String = db
            .query_row(
                "SELECT tag FROM generations WHERE gen_id = ?1",
                &[&gen],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(tag, format!("gen-tag-{}", gen));
    }
}

#[test]
fn test_empirical_crypto_envelope_nonce_uniqueness_across_flushes() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("nonce_audit.db");
    let key = [0x77u8; 32];

    let db = EncryptedDatabase::open(&db_path, key).unwrap();
    db.execute_batch("CREATE TABLE items (id INT); INSERT INTO items VALUES (1);")
        .unwrap();

    let mut observed_nonces = HashSet::new();
    let mut observed_ciphertexts = HashSet::new();

    // Flush 25 times with identical database contents
    for _ in 0..25 {
        db.flush().unwrap();
        let raw_bytes = std::fs::read(&db_path).unwrap();
        assert!(raw_bytes.len() >= MIN_ENVELOPE_LEN);

        let nonce = raw_bytes[5..17].to_vec();
        let ciphertext = raw_bytes[17..].to_vec();

        assert!(
            observed_nonces.insert(nonce),
            "FATAL CRYPTOGRAPHIC FLAW: Nonce reuse detected in AES-256-GCM flush!"
        );
        assert!(
            observed_ciphertexts.insert(ciphertext),
            "Ciphertexts must be distinct due to fresh nonces"
        );
    }

    assert_eq!(observed_nonces.len(), 25);
}

#[test]
fn test_empirical_disk_byte_forensics_and_shannon_entropy() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("forensic_check.db");
    let svid = Svid::new("spiffe://aura.local/agent/forensic-auditor");

    let canary_task_id = "canary-super-secret-task-uuid-8877";
    let canary_secret = "Bearer sk-proj-super-secret-authorization-token-9988";
    let canary_table = "confidential_workload_records";

    {
        let ledger = TaskLedger::open_with_svid(&db_path, &svid).unwrap();

        let mut meta = HashMap::new();
        meta.insert("token".to_string(), canary_secret.to_string());
        meta.insert("endpoint".to_string(), "https://vault.corp.internal".to_string());

        let event = TaskEvent::new(canary_task_id, TaskState::Running, 1728700000)
            .with_json_metadata(&meta)
            .with_transition_reason("Executing highly sensitive microVM command");

        ledger.append_event(&event).unwrap();

        // Also add custom table with custom secret
        ledger
            .db()
            .execute_batch(&format!(
                "CREATE TABLE {} (id INT, secret TEXT); INSERT INTO {} VALUES (1, '{}');",
                canary_table, canary_table, canary_secret
            ))
            .unwrap();
        ledger.flush().unwrap();
    }

    // Read raw disk bytes directly
    let raw_bytes = std::fs::read(&db_path).expect("Failed to read raw DB file");
    assert!(raw_bytes.len() >= MIN_ENVELOPE_LEN);

    // 1. Header verification
    assert_eq!(&raw_bytes[0..4], ENCRYPTED_MAGIC);
    assert_eq!(raw_bytes[4], 1);

    // 2. Scan entire raw byte slice for SQLite plaintext signatures
    let subslice_contains = |needle: &[u8]| raw_bytes.windows(needle.len()).any(|w| w == needle);

    assert!(
        !subslice_contains(b"SQLite format 3"),
        "LEAK: 'SQLite format 3' header leaked onto disk!"
    );
    assert!(
        !subslice_contains(b"SQLite"),
        "LEAK: 'SQLite' string leaked onto disk!"
    );
    assert!(
        !subslice_contains(canary_task_id.as_bytes()),
        "LEAK: Task ID leaked in plaintext onto disk!"
    );
    assert!(
        !subslice_contains(canary_secret.as_bytes()),
        "LEAK: Sensitive bearer token leaked in plaintext onto disk!"
    );
    assert!(
        !subslice_contains(canary_table.as_bytes()),
        "LEAK: Table name leaked in plaintext onto disk!"
    );
    assert!(
        !subslice_contains(b"task_events"),
        "LEAK: Schema table 'task_events' leaked in plaintext onto disk!"
    );
    assert!(
        !subslice_contains(b"CREATE TABLE"),
        "LEAK: DDL 'CREATE TABLE' leaked in plaintext onto disk!"
    );

    // 3. Shannon Entropy verification on ciphertext (offset 17 to end)
    let ciphertext_and_tag = &raw_bytes[17..];
    let entropy = shannon_entropy(ciphertext_and_tag);
    println!(
        "Ciphertext length: {} bytes, Shannon Entropy: {:.4} bits/byte",
        ciphertext_and_tag.len(),
        entropy
    );

    // AES-256-GCM ciphertext exhibits high entropy (~7.8 - 8.0 bits/byte for authentic ciphertext)
    assert!(
        entropy > 7.5,
        "Entropy too low ({:.4}): ciphertext does not appear properly encrypted",
        entropy
    );
}

#[test]
fn test_empirical_tampering_and_bit_flipping_adversarial_rejections() {
    let key = [0x5Au8; 32];
    let nonce = [0x11u8; NONCE_LEN];
    let payload = b"Production database tables and transaction logs";

    let original = CryptoEnvelope::encrypt(&key, &nonce, payload).unwrap();

    // 1. Invalid Magic Bytes
    let mut tampered = original.clone();
    tampered[0] = b'B';
    let err = CryptoEnvelope::decrypt(&key, &tampered).unwrap_err();
    assert!(matches!(err, StorageError::InvalidMagic));

    // 2. Unsupported Version
    let mut tampered = original.clone();
    tampered[4] = 99;
    let err = CryptoEnvelope::decrypt(&key, &tampered).unwrap_err();
    assert!(matches!(err, StorageError::UnsupportedVersion(99)));

    // 3. Truncated envelopes (< MIN_ENVELOPE_LEN 33 bytes)
    for len in [0, 1, 4, 5, 16, 17, 32] {
        let err = CryptoEnvelope::decrypt(&key, &original[..len]).unwrap_err();
        assert!(matches!(err, StorageError::EnvelopeTooShort));
    }

    // 4. Single-bit flips across nonce (bytes 5..17)
    for idx in 5..17 {
        let mut tampered = original.clone();
        tampered[idx] ^= 0x01;
        let err = CryptoEnvelope::decrypt(&key, &tampered).unwrap_err();
        assert!(
            matches!(
                err,
                StorageError::AuthenticationError(_) | StorageError::AuthenticationFailed(_)
            ),
            "Nonce byte {} bit flip must fail authentication",
            idx
        );
    }

    // 5. Single-bit flips across ciphertext (bytes 17..original.len() - 16)
    for idx in 17..original.len() - 16 {
        let mut tampered = original.clone();
        tampered[idx] ^= 0x04;
        let err = CryptoEnvelope::decrypt(&key, &tampered).unwrap_err();
        assert!(
            matches!(
                err,
                StorageError::AuthenticationError(_) | StorageError::AuthenticationFailed(_)
            ),
            "Ciphertext byte {} bit flip must fail authentication",
            idx
        );
    }

    // 6. Single-bit flips across authentication tag (last 16 bytes)
    for idx in original.len() - 16..original.len() {
        let mut tampered = original.clone();
        tampered[idx] ^= 0x80;
        let err = CryptoEnvelope::decrypt(&key, &tampered).unwrap_err();
        assert!(
            matches!(
                err,
                StorageError::AuthenticationError(_) | StorageError::AuthenticationFailed(_)
            ),
            "Auth tag byte {} bit flip must fail authentication",
            idx
        );
    }

    // 7. SVID Orthogonality: different SPIFFE ID must strictly fail decryption
    let svid_1 = Svid::new("spiffe://aura.local/agent/workload-1");
    let svid_2 = Svid::new("spiffe://aura.local/agent/workload-2");
    let key_1 = derive_storage_key(&svid_1);
    let key_2 = derive_storage_key(&svid_2);

    let env_1 = CryptoEnvelope::encrypt(&key_1, &nonce, payload).unwrap();
    let decrypt_with_key_2 = CryptoEnvelope::decrypt(&key_2, &env_1);
    assert!(decrypt_with_key_2.is_err());
}

#[test]
fn test_empirical_task_ledger_high_volume_concurrency_stress() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("ledger_concurrency.db");
    let svid = Svid::new("spiffe://aura.local/agent/concurrent-ledger");

    let ledger = Arc::new(TaskLedger::open_with_svid(&db_path, &svid).unwrap());
    let num_tasks = 20;
    let events_per_task = 10;

    let mut handles = Vec::new();

    for t_idx in 0..num_tasks {
        let ledger_clone = Arc::clone(&ledger);
        let task_id = format!("task-stress-{:03}", t_idx);

        let handle = std::thread::spawn(move || {
            let states = [
                TaskState::Pending,
                TaskState::Submitted,
                TaskState::Dispatched,
                TaskState::Running,
                TaskState::Completed,
            ];

            for e_idx in 0..events_per_task {
                let state = states[e_idx % states.len()].clone();
                let event = TaskEvent::new(&task_id, state, 1728700000 + e_idx as u64)
                    .with_metadata(format!("{{\"iter\": {}}}", e_idx))
                    .with_transition_reason(format!("Transition step {}", e_idx));

                ledger_clone
                    .append_event(&event)
                    .unwrap_or_else(|e| panic!("Thread append failed: {:?}", e));
            }
        });
        handles.push(handle);
    }

    for h in handles {
        h.join().expect("Worker thread panicked");
    }

    // Verify all 200 events are present and queryable
    for t_idx in 0..num_tasks {
        let task_id = format!("task-stress-{:03}", t_idx);
        let events = ledger.get_events(&task_id).unwrap();
        assert_eq!(
            events.len(),
            events_per_task,
            "Task {} must have exactly {} events",
            task_id,
            events_per_task
        );

        for (e_idx, event) in events.iter().enumerate() {
            assert_eq!(event.task_id, task_id);
            assert_eq!(event.timestamp, 1728700000 + e_idx as u64);
        }
    }
}

// ============================================================================
// AREA 3: Feature 13 - Vsock Stream Memory Bounds & Truncation
// ============================================================================

#[tokio::test]
async fn test_empirical_vsock_stream_memory_limits_and_instant_truncation() {
    // 1. Exact boundary: 512 bytes with 512-byte limit -> not truncated
    let exact_data = vec![0x33; 512];
    let mut cursor = std::io::Cursor::new(exact_data.clone());
    let (buf, truncated) = read_stream_with_limit_ext(&mut cursor, 512)
        .await
        .unwrap();
    assert_eq!(buf, exact_data);
    assert!(!truncated);

    // 2. Limit + 1 byte: 513 bytes with 512-byte limit -> truncated at 512 bytes
    let overflow_data = vec![0x44; 513];
    let mut cursor = std::io::Cursor::new(overflow_data);
    let (buf, truncated) = read_stream_with_limit_ext(&mut cursor, 512)
        .await
        .unwrap();
    assert_eq!(buf.len(), 512);
    assert!(truncated);

    // 3. Huge rogue payload: 10 MiB stream with 8 KiB limit -> truncated without OOM
    let rogue_payload = vec![0x55; 10 * 1024 * 1024];
    let mut cursor = std::io::Cursor::new(rogue_payload);
    let (buf, truncated) = read_stream_with_limit_ext(&mut cursor, 8192)
        .await
        .unwrap();
    assert_eq!(buf.len(), 8192);
    assert!(truncated);

    // 4. Zero byte limit -> instant 0-byte return with truncated = true
    let data = vec![0x66; 100];
    let mut cursor = std::io::Cursor::new(data);
    let (buf, truncated) = read_stream_with_limit_ext(&mut cursor, 0)
        .await
        .unwrap();
    assert_eq!(buf.len(), 0);
    assert!(truncated);

    // 5. Default VsockStreamConfig helper
    let config = VsockStreamConfig::default();
    assert_eq!(config.max_buffer_bytes, 16 * 1024 * 1024);
}
