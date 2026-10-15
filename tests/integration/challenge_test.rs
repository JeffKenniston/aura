//! Empirical Challenge & Stress Test Suite for Milestone 3 (Features 13-17)
//!
//! Author: challenger_m3_1 (Empirical Challenger)
//! Roles: critic, specialist
//!
//! Target Verification Areas:
//! 1. Feature 13: Vsock stream buffer limits (read_stream_with_limit_ext, read_stream_with_limit)
//!    - Infinite stream instant truncation without OOM or hang
//!    - Large 100MB stream bounded memory and short-circuit verification
//!    - Exact boundary ladder (N-1, N, N+1, chunk boundaries 64KB, 128KB)
//!    - Chunked drip stream truncation
//! 2. Features 15 & 17: AES-256-GCM EncryptedDatabase and SPIFFE key derivation
//!    - Raw disk inspection: 100% cryptographic opacity across 20+ sensitive needles
//!    - High Shannon entropy verification of raw disk ciphertext
//!    - Tamper resilience matrix: magic bytes, version, nonce, ciphertext bit flips, tag corruption, envelope truncation
//!    - SVID key derivation orthogonality and Hamming distance dispersion
//! 3. Feature 16: TaskLedger persistence and multi-event lifecycle
//!    - Full lifecycle transitions across all standard and custom states
//!    - Interleaved multi-task isolation across 10 tasks with zero cross-talk
//!    - Heavy 256KB JSON metadata persistence and opacity validation

use aura_host::hypervisor::firecracker::{
    read_stream_with_limit, read_stream_with_limit_ext,
};
use aura_host::identity::Svid;
use aura_host::storage::{
    derive_storage_key, derive_storage_key_with_context, EncryptedDatabase,
    StorageError, TaskEvent, TaskLedger, TaskState, ENCRYPTED_MAGIC, ENCRYPTED_VERSION,
    MIN_ENVELOPE_LEN, TAG_LEN,
};
use std::io::Cursor;
use std::pin::Pin;
use std::task::{Context, Poll};
use tempfile::tempdir;
use tokio::io::{AsyncRead, ReadBuf};

// ============================================================================
// Helpers & Mock Generators
// ============================================================================

/// Infinite byte generator that never terminates, producing a constant byte.
struct InfiniteByteStream {
    byte: u8,
    bytes_produced: usize,
}

impl InfiniteByteStream {
    fn new(byte: u8) -> Self {
        Self {
            byte,
            bytes_produced: 0,
        }
    }
}

impl AsyncRead for InfiniteByteStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let remaining = buf.remaining();
        if remaining > 0 {
            let slice = vec![self.byte; remaining];
            buf.put_slice(&slice);
            self.bytes_produced += remaining;
        }
        Poll::Ready(Ok(()))
    }
}

/// Bounded byte generator that simulates a huge payload up to `total_bytes`
/// but tracks exactly how many bytes were actually pulled by the reader.
struct TrackedByteStream {
    total_bytes: usize,
    bytes_read: usize,
}

impl TrackedByteStream {
    fn new(total_bytes: usize) -> Self {
        Self {
            total_bytes,
            bytes_read: 0,
        }
    }
}

impl AsyncRead for TrackedByteStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let available = self.total_bytes - self.bytes_read;
        if available == 0 {
            return Poll::Ready(Ok(()));
        }

        let to_write = buf.remaining().min(available);
        let chunk = vec![0xABu8; to_write];
        buf.put_slice(&chunk);
        self.bytes_read += to_write;

        Poll::Ready(Ok(()))
    }
}

/// Drip stream producing small chunks of `chunk_size` on each poll.
struct DripStream {
    remaining: usize,
    chunk_size: usize,
}

impl DripStream {
    fn new(total: usize, chunk_size: usize) -> Self {
        Self {
            remaining: total,
            chunk_size,
        }
    }
}

impl AsyncRead for DripStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        if self.remaining == 0 {
            return Poll::Ready(Ok(()));
        }

        let to_give = self.remaining.min(self.chunk_size).min(buf.remaining());
        let slice = vec![0x7Eu8; to_give];
        buf.put_slice(&slice);
        self.remaining -= to_give;

        Poll::Ready(Ok(()))
    }
}

/// Subslice search helper.
fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// Computes Shannon entropy of a byte slice in bits per byte (max 8.0).
fn shannon_entropy(bytes: &[u8]) -> f64 {
    if bytes.is_empty() {
        return 0.0;
    }
    let mut counts = [0usize; 256];
    for &b in bytes {
        counts[b as usize] += 1;
    }
    let len = bytes.len() as f64;
    let mut entropy = 0.0;
    for &c in &counts {
        if c > 0 {
            let p = c as f64 / len;
            entropy -= p * p.log2();
        }
    }
    entropy
}

// ============================================================================
// PART 1: Feature 13 - Vsock Stream Buffer Limits Empirical Stress
// ============================================================================

#[tokio::test]
async fn test_challenge_f13_infinite_stream_instant_truncation() {
    let mut stream = InfiniteByteStream::new(0x5A);
    const LIMIT: usize = 8192;

    let (buf, truncated) = read_stream_with_limit_ext(&mut stream, LIMIT)
        .await
        .expect("read_stream_with_limit_ext failed on infinite stream");

    assert!(truncated, "Infinite stream must trigger truncation flag");
    assert_eq!(buf.len(), LIMIT, "Buffer must be capped at exactly LIMIT");
    assert!(
        buf.iter().all(|&b| b == 0x5A),
        "All bytes must match generator content"
    );
    // Ensure memory consumption remained bounded (produced only LIMIT + small read ahead)
    assert!(
        stream.bytes_produced <= LIMIT + 65536,
        "Stream reader consumed too much data before truncating: {} bytes",
        stream.bytes_produced
    );
}

#[tokio::test]
async fn test_challenge_f13_large_100mb_stream_short_circuit() {
    const TOTAL_PAYLOAD: usize = 100 * 1024 * 1024; // 100 MB
    const LIMIT: usize = 64 * 1024; // 64 KB limit

    let mut stream = TrackedByteStream::new(TOTAL_PAYLOAD);

    let (buf, truncated) = read_stream_with_limit_ext(&mut stream, LIMIT)
        .await
        .expect("read_stream_with_limit_ext failed on 100MB stream");

    assert!(truncated, "100MB stream must be truncated");
    assert_eq!(buf.len(), LIMIT, "Output buffer must equal limit");

    // The reader must short-circuit and NOT consume the 100MB payload!
    assert!(
        stream.bytes_read <= LIMIT + 65536,
        "Rogue stream consumed {} bytes, expected short-circuit around LIMIT",
        stream.bytes_read
    );
}

#[tokio::test]
async fn test_challenge_f13_exact_boundary_ladder() {
    // Tests: limit-1, limit, limit+1 across multiple chunk boundaries
    let limits_to_test = vec![0, 1, 1024, 65535, 65536, 65537, 131072];

    for limit in limits_to_test {
        // Test 1: exact limit
        {
            let data = vec![0x33u8; limit];
            let mut cursor = Cursor::new(data.clone());
            let (buf, truncated) = read_stream_with_limit_ext(&mut cursor, limit)
                .await
                .expect("exact boundary failed");
            assert_eq!(
                truncated, false,
                "Stream with exact limit length should not be marked truncated (limit={})",
                limit
            );
            assert_eq!(buf.len(), limit);
            assert_eq!(buf, data);
        }

        // Test 2: limit + 1 (should truncate if limit > 0)
        {
            let data = vec![0x44u8; limit + 1];
            let mut cursor = Cursor::new(data.clone());
            let (buf, truncated) = read_stream_with_limit_ext(&mut cursor, limit)
                .await
                .expect("limit+1 boundary failed");
            assert!(
                truncated,
                "Stream exceeding limit by 1 byte must be marked truncated (limit={})",
                limit
            );
            assert_eq!(buf.len(), limit);
            assert_eq!(&buf[..], &data[..limit]);
        }

        // Test 3: limit - 1 (if limit > 0)
        if limit > 0 {
            let data = vec![0x55u8; limit - 1];
            let mut cursor = Cursor::new(data.clone());
            let (buf, truncated) = read_stream_with_limit_ext(&mut cursor, limit)
                .await
                .expect("limit-1 boundary failed");
            assert!(
                !truncated,
                "Stream with limit-1 bytes must not be marked truncated"
            );
            assert_eq!(buf.len(), limit - 1);
            assert_eq!(buf, data);
        }
    }
}

#[tokio::test]
async fn test_challenge_f13_drip_stream_truncation() {
    // 5000 bytes dripped in 23-byte chunks, with limit = 350
    let mut drip = DripStream::new(5000, 23);
    const LIMIT: usize = 350;

    let buf = read_stream_with_limit(&mut drip, LIMIT)
        .await
        .expect("drip stream read failed");

    assert_eq!(buf.len(), LIMIT, "Drip stream must be truncated at LIMIT");
    assert!(buf.iter().all(|&b| b == 0x7E));
}

// ============================================================================
// PART 2: Features 15 & 17 - EncryptedDatabase, SPIFFE KDF, Raw Disk Opacity
// ============================================================================

#[test]
fn test_challenge_f15_f17_raw_disk_opacity_comprehensive_scan() {
    let dir = tempdir().expect("tempdir failed");
    let db_path = dir.path().join("secure_vault.db");
    let svid = Svid::new("spiffe://aura.local/agent/emp-challenger-01");
    let key = derive_storage_key(&svid);

    // 1. Populate database with sensitive schema and records
    {
        let db = EncryptedDatabase::open(&db_path, key).expect("open failed");
        db.execute_batch(
            "CREATE TABLE credentials (
                service TEXT PRIMARY KEY,
                access_token TEXT NOT NULL,
                secret_key TEXT NOT NULL,
                spiffe_uri TEXT NOT NULL
            );
            CREATE TABLE audit_logs (
                id INTEGER PRIMARY KEY,
                action TEXT,
                caller TEXT,
                statement TEXT
            );
            INSERT INTO credentials VALUES (
                'github.com',
                'ghp_SUPER_SECRET_GITHUB_PAT_998877665544',
                'sk-live-5544332211AABBCCDDEEFF00',
                'spiffe://aura.local/ns/prod/sa/admin'
            );
            INSERT INTO audit_logs VALUES (
                1,
                'GRANT_ACCESS',
                'admin_agent',
                'SELECT * FROM credentials WHERE service = \"github.com\"'
            );",
        )
        .expect("execute_batch failed");

        db.flush().expect("flush failed");
    }

    // 2. Read raw bytes from physical disk
    let raw_bytes = std::fs::read(&db_path).expect("fs::read db failed");
    assert!(
        raw_bytes.len() >= MIN_ENVELOPE_LEN,
        "Database envelope must be at least {} bytes, got {}",
        MIN_ENVELOPE_LEN,
        raw_bytes.len()
    );

    // 3. Header verification
    assert_eq!(
        &raw_bytes[0..4],
        ENCRYPTED_MAGIC,
        "Disk file must start with b'AURA'"
    );
    assert_eq!(
        raw_bytes[4], ENCRYPTED_VERSION,
        "Disk file version must be 1"
    );

    // 4. Comprehensive Blacklist Scan
    let blacklisted_needles: Vec<&[u8]> = vec![
        b"SQLite format 3\0",
        b"SQLite format 3",
        b"SQLite format",
        b"credentials",
        b"audit_logs",
        b"access_token",
        b"secret_key",
        b"spiffe_uri",
        b"ghp_SUPER_SECRET_GITHUB_PAT",
        b"sk-live-5544332211",
        b"spiffe://aura.local/ns/prod/sa/admin",
        b"spiffe://aura.local",
        b"GRANT_ACCESS",
        b"admin_agent",
        b"CREATE TABLE",
        b"INSERT INTO",
        b"SELECT * FROM",
        b"WHERE service",
        b"github.com",
    ];

    for needle in blacklisted_needles {
        let needle_str = String::from_utf8_lossy(needle);
        assert!(
            !contains_subslice(&raw_bytes, needle),
            "CRITICAL SECURITY DEFECT: Plaintext needle '{}' detected in raw disk file!",
            needle_str
        );
    }

    // 5. High Shannon Entropy Verification on Ciphertext
    // Nonce is 12 bytes at [5..17], ciphertext + tag is at [17..]
    let ciphertext_body = &raw_bytes[17..];
    let entropy = shannon_entropy(ciphertext_body);
    assert!(
        entropy > 7.7,
        "Ciphertext Shannon entropy too low: {:.4} bits/byte (expected > 7.7)",
        entropy
    );

    // 6. Legitimate Decryption and Data Verification
    {
        let db = EncryptedDatabase::open(&db_path, key).expect("reopen failed");
        let token: String = db
            .query_row(
                "SELECT access_token FROM credentials WHERE service = 'github.com'",
                [],
                |row| row.get(0),
            )
            .expect("query_row failed");
        assert_eq!(token, "ghp_SUPER_SECRET_GITHUB_PAT_998877665544");
    }
}

#[test]
fn test_challenge_f15_tamper_resilience_matrix() {
    let dir = tempdir().expect("tempdir failed");
    let original_path = dir.path().join("original.db");
    let key = [0x77u8; 32];

    // Create valid encrypted DB
    {
        let db = EncryptedDatabase::open(&original_path, key).expect("open failed");
        db.execute_batch("CREATE TABLE t (x INT); INSERT INTO t VALUES (42);")
            .unwrap();
        db.flush().unwrap();
    }
    let original_bytes = std::fs::read(&original_path).expect("read original failed");

    // Matrix 1: Magic Byte Tampering
    {
        let mut tampered = original_bytes.clone();
        tampered[0] = b'B'; // "BURA" instead of "AURA"
        let p = dir.path().join("bad_magic.db");
        std::fs::write(&p, tampered).unwrap();
        let res = EncryptedDatabase::open(&p, key);
        assert!(matches!(res.unwrap_err(), StorageError::InvalidMagic));
    }

    // Matrix 2: Version Byte Tampering
    {
        for bad_version in [0u8, 2u8, 255u8] {
            let mut tampered = original_bytes.clone();
            tampered[4] = bad_version;
            let p = dir.path().join(format!("bad_version_{}.db", bad_version));
            std::fs::write(&p, tampered).unwrap();
            let res = EncryptedDatabase::open(&p, key);
            assert!(matches!(
                res.unwrap_err(),
                StorageError::UnsupportedVersion(v) if v == bad_version
            ));
        }
    }

    // Matrix 3: Truncated Envelope Lengths (< MIN_ENVELOPE_LEN)
    {
        for len in [0, 1, 4, 16, 32] {
            let tampered = &original_bytes[..len.min(original_bytes.len())];
            let p = dir.path().join(format!("truncated_{}.db", len));
            std::fs::write(&p, tampered).unwrap();
            let res = EncryptedDatabase::open(&p, key);
            assert!(
                matches!(res.unwrap_err(), StorageError::EnvelopeTooShort),
                "Length {} must return EnvelopeTooShort",
                len
            );
        }
    }

    // Matrix 4: Nonce Bit Flipping (indices 5..17)
    {
        for nonce_idx in 5..17 {
            let mut tampered = original_bytes.clone();
            tampered[nonce_idx] ^= 0x01;
            let p = dir.path().join(format!("bad_nonce_{}.db", nonce_idx));
            std::fs::write(&p, tampered).unwrap();
            let res = EncryptedDatabase::open(&p, key);
            assert!(
                matches!(
                    res.unwrap_err(),
                    StorageError::AuthenticationError(_) | StorageError::AuthenticationFailed(_)
                ),
                "Tampered nonce byte at index {} must fail authentication",
                nonce_idx
            );
        }
    }

    // Matrix 5: Ciphertext Bit Flipping (multiple points)
    {
        let points = [17, original_bytes.len() / 2, original_bytes.len() - 17];
        for &ct_idx in &points {
            let mut tampered = original_bytes.clone();
            tampered[ct_idx] ^= 0x40;
            let p = dir.path().join(format!("bad_ct_{}.db", ct_idx));
            std::fs::write(&p, tampered).unwrap();
            let res = EncryptedDatabase::open(&p, key);
            assert!(
                matches!(
                    res.unwrap_err(),
                    StorageError::AuthenticationError(_) | StorageError::AuthenticationFailed(_)
                ),
                "Flipped ciphertext bit at index {} must fail authentication",
                ct_idx
            );
        }
    }

    // Matrix 6: Auth Tag Bit Flipping (last 16 bytes)
    {
        let last_16_start = original_bytes.len() - TAG_LEN;
        for tag_idx in last_16_start..original_bytes.len() {
            let mut tampered = original_bytes.clone();
            tampered[tag_idx] ^= 0x80;
            let p = dir.path().join(format!("bad_tag_{}.db", tag_idx));
            std::fs::write(&p, tampered).unwrap();
            let res = EncryptedDatabase::open(&p, key);
            assert!(
                matches!(
                    res.unwrap_err(),
                    StorageError::AuthenticationError(_) | StorageError::AuthenticationFailed(_)
                ),
                "Flipped auth tag byte at index {} must fail authentication",
                tag_idx
            );
        }
    }

    // Matrix 7: Unauthorized Key Rejection
    {
        let wrong_key = [0x99u8; 32];
        let res = EncryptedDatabase::open(&original_path, wrong_key);
        assert!(
            matches!(
                res.unwrap_err(),
                StorageError::AuthenticationError(_) | StorageError::AuthenticationFailed(_)
            ),
            "Wrong key must fail authentication"
        );
    }
}

#[test]
fn test_challenge_f17_spiffe_kdf_dispersion() {
    let svid_base = Svid::new("spiffe://aura.local/agent/service-alpha-001");
    let svid_diff_1char = Svid::new("spiffe://aura.local/agent/service-alpha-002");
    let svid_other_domain = Svid::new("spiffe://corp.enterprise.com/agent/service-alpha-001");

    let key_base = derive_storage_key(&svid_base);
    let key_diff_1char = derive_storage_key(&svid_diff_1char);
    let key_other_domain = derive_storage_key(&svid_other_domain);

    // Verify deterministic repeatability
    assert_eq!(derive_storage_key(&svid_base), key_base);

    // Count bit differences (Hamming distance) across 256 bits
    fn hamming_distance(a: &[u8; 32], b: &[u8; 32]) -> u32 {
        a.iter()
            .zip(b.iter())
            .map(|(&x, &y)| (x ^ y).count_ones())
            .sum()
    }

    let dist_1char = hamming_distance(&key_base, &key_diff_1char);
    let dist_domain = hamming_distance(&key_base, &key_other_domain);

    // Ideal avalanche effect: Hamming distance should be close to 128 (out of 256)
    assert!(
        dist_1char >= 90 && dist_1char <= 166,
        "Hamming distance for 1-char SVID diff must be well-dispersed, got {}",
        dist_1char
    );
    assert!(
        dist_domain >= 90 && dist_domain <= 166,
        "Hamming distance for different domain must be well-dispersed, got {}",
        dist_domain
    );

    // Test context string separation
    let key_ctx1 = derive_storage_key_with_context(&svid_base, "ctx-app-1");
    let key_ctx2 = derive_storage_key_with_context(&svid_base, "ctx-app-2");
    assert_ne!(key_ctx1, key_ctx2);
    let dist_ctx = hamming_distance(&key_ctx1, &key_ctx2);
    assert!(dist_ctx >= 90 && dist_ctx <= 166);
}

// ============================================================================
// PART 3: Feature 16 - TaskLedger Lifecycle & Multi-Task Persistence
// ============================================================================

#[test]
fn test_challenge_f16_full_lifecycle_state_transitions() {
    let dir = tempdir().expect("tempdir failed");
    let ledger_path = dir.path().join("task_lifecycle.db");
    let key = [0x55u8; 32];

    let task_id = "lifecycle-task-101";

    let transitions = vec![
        (TaskState::Pending, 1000, "Enqueued in task pool"),
        (TaskState::Submitted, 1005, "Scheduled by supervisor"),
        (TaskState::Dispatched, 1010, "Dispatched to Firecracker microVM"),
        (TaskState::Running, 1015, "Executing payload entrypoint"),
        (TaskState::Failed, 1030, "Process crashed with exit code 137 (OOM)"),
        (TaskState::Custom("remediated".into()), 1040, "Restarted with 2x memory"),
        (TaskState::Completed, 1060, "Execution completed successfully"),
    ];

    // 1. Append transitions to ledger
    {
        let ledger = TaskLedger::open(&ledger_path, key).expect("open ledger failed");
        for (state, ts, reason) in &transitions {
            let event = TaskEvent::new(task_id, state.clone(), *ts)
                .with_transition_reason(*reason)
                .with_metadata(format!("{{\"step\": {}}}", ts));
            ledger.append_event(&event).expect("append_event failed");
        }
    }

    // 2. Re-open ledger and verify strict chronological order and fidelity
    {
        let ledger = TaskLedger::open(&ledger_path, key).expect("reopen ledger failed");
        let events = ledger.get_events(task_id).expect("get_events failed");

        assert_eq!(events.len(), transitions.len());
        for (idx, (state, ts, reason)) in transitions.iter().enumerate() {
            assert_eq!(&events[idx].state, state);
            assert_eq!(events[idx].timestamp, *ts);
            assert_eq!(events[idx].transition_reason.as_deref(), Some(*reason));
            assert_eq!(
                events[idx].metadata.as_deref(),
                Some(format!("{{\"step\": {}}}", ts).as_str())
            );
        }
    }
}

#[test]
fn test_challenge_f16_multi_task_interleaving_isolation() {
    let dir = tempdir().expect("tempdir failed");
    let ledger_path = dir.path().join("multi_task.db");
    let key = [0x88u8; 32];

    const TASK_COUNT: usize = 10;
    let states = vec![
        TaskState::Pending,
        TaskState::Dispatched,
        TaskState::Running,
        TaskState::Completed,
    ];

    // Append events interleaved across tasks
    {
        let ledger = TaskLedger::open(&ledger_path, key).expect("open ledger failed");
        for (step_idx, state) in states.iter().enumerate() {
            for task_num in 0..TASK_COUNT {
                let task_id = format!("task-{:03}", task_num);
                let ts = (1000 + step_idx * 10 + task_num) as u64;
                let event = TaskEvent::new(&task_id, state.clone(), ts)
                    .with_transition_reason(format!("Step {} for {}", step_idx, task_id));
                ledger.append_event(&event).expect("append failed");
            }
        }
    }

    // Verify isolation: query each task and ensure exactly its 4 events are returned
    {
        let ledger = TaskLedger::open(&ledger_path, key).expect("reopen failed");
        for task_num in 0..TASK_COUNT {
            let task_id = format!("task-{:03}", task_num);
            let events = ledger.get_events(&task_id).expect("get_events failed");
            assert_eq!(
                events.len(),
                states.len(),
                "Task {} should have exactly {} events",
                task_id,
                states.len()
            );

            for (step_idx, expected_state) in states.iter().enumerate() {
                assert_eq!(&events[step_idx].state, expected_state);
                assert_eq!(
                    events[step_idx].transition_reason.as_deref(),
                    Some(format!("Step {} for {}", step_idx, task_id).as_str())
                );
            }
        }
    }
}

#[test]
fn test_challenge_f16_heavy_payload_persistence_stress() {
    let dir = tempdir().expect("tempdir failed");
    let ledger_path = dir.path().join("heavy_stress.db");
    let key = [0xAAu8; 32];

    // Create a 256 KB JSON metadata payload
    let large_json = format!(
        "{{\"large_stacktrace\": \"{}\", \"heap_dump\": \"{}\"}}",
        "A".repeat(128 * 1024),
        "B".repeat(128 * 1024)
    );
    assert!(large_json.len() >= 256 * 1024);

    let task_id = "stress-heavy-task-001";
    let event = TaskEvent::new(task_id, TaskState::Running, 999999)
        .with_metadata(&large_json)
        .with_transition_reason("Heavy stress payload test");

    // Append and flush
    {
        let ledger = TaskLedger::open(&ledger_path, key).expect("open failed");
        ledger.append_event(&event).expect("append heavy event failed");
    }

    // Reopen and assert 100% data fidelity
    {
        let ledger = TaskLedger::open(&ledger_path, key).expect("reopen failed");
        let events = ledger.get_events(task_id).expect("get_events failed");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].metadata.as_ref(), Some(&large_json));
        assert_eq!(
            events[0].transition_reason.as_deref(),
            Some("Heavy stress payload test")
        );
    }

    // Raw disk opacity check on the 256KB+ database
    let raw_bytes = std::fs::read(&ledger_path).expect("read raw heavy db failed");
    assert!(
        raw_bytes.len() >= 256 * 1024,
        "Database on disk must reflect payload size"
    );
    assert_eq!(&raw_bytes[0..4], ENCRYPTED_MAGIC);

    // Verify no plaintext strings from heavy payload touch disk
    assert!(
        !contains_subslice(&raw_bytes, b"large_stacktrace"),
        "Raw disk contains plaintext JSON key 'large_stacktrace'"
    );
    assert!(
        !contains_subslice(&raw_bytes, b"heap_dump"),
        "Raw disk contains plaintext JSON key 'heap_dump'"
    );
    assert!(
        !contains_subslice(&raw_bytes, &vec![b'A'; 1000]),
        "Raw disk contains plaintext run of 1000 'A' characters"
    );
    assert!(
        !contains_subslice(&raw_bytes, &vec![b'B'; 1000]),
        "Raw disk contains plaintext run of 1000 'B' characters"
    );
}
