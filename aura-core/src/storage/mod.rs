//! Encrypted Storage Subsystem for Aura OS (Features 15, 16, 17)
//!
//! Provides identity-locked, AES-256-GCM envelope-encrypted persistence
//! over SQLite databases and Task Ledgers.

pub mod encrypted;
pub mod ledger;

pub use encrypted::{
    derive_storage_key, derive_storage_key_with_context, CryptoEnvelope, EncryptedDatabase,
    StorageError, ENCRYPTED_MAGIC, ENCRYPTED_VERSION, HEADER_LEN, KDF_STORAGE_CONTEXT,
    MIN_ENVELOPE_LEN, NONCE_LEN, TAG_LEN,
};
pub use ledger::{TaskEvent, TaskLedger, TaskState};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::Svid;
    use std::collections::HashMap;
    use tempfile::tempdir;

    fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
        if needle.is_empty() {
            return true;
        }
        haystack.windows(needle.len()).any(|w| w == needle)
    }

    #[test]
    fn test_spiffe_kdf_key_derivation() {
        let svid_a = Svid::new("spiffe://aura.local/agent/workload-alpha");
        let svid_b = Svid::new("spiffe://aura.local/agent/workload-beta");

        let key_a1 = derive_storage_key(&svid_a);
        let key_a2 = derive_storage_key(&svid_a);
        let key_b = derive_storage_key(&svid_b);

        assert_eq!(key_a1, key_a2, "BLAKE3 KDF must be strictly deterministic for identical SVID");
        assert_ne!(key_a1, key_b, "Distinct SVIDs must yield orthogonal 256-bit keys");
        assert_ne!(key_a1, [0u8; 32], "Derived key must not be all zeros");
    }

    #[test]
    fn test_roundtrip_envelope_encryption_decryption() {
        let key = [0x42u8; 32];
        let nonce = [0x17u8; NONCE_LEN];
        let payload = b"Sensitive SQLite page data containing user tables";

        let envelope = CryptoEnvelope::encrypt(&key, &nonce, payload).expect("encrypt failed");
        assert_eq!(&envelope[0..4], b"AURA", "Magic must be b'AURA'");
        assert_eq!(envelope[4], 1, "Version must be 1");
        assert_eq!(&envelope[5..17], &nonce, "Nonce must match");

        let decrypted = CryptoEnvelope::decrypt(&key, &envelope).expect("decrypt failed");
        assert_eq!(decrypted, payload, "Decrypted payload must match original");
    }

    #[test]
    fn test_empty_plaintext_envelope() {
        let key = [0x55u8; 32];
        let nonce = [0x66u8; NONCE_LEN];
        let envelope = CryptoEnvelope::encrypt(&key, &nonce, b"").expect("encrypt empty failed");

        assert_eq!(envelope.len(), MIN_ENVELOPE_LEN, "Empty envelope must be exactly 33 bytes");
        let decrypted = CryptoEnvelope::decrypt(&key, &envelope).expect("decrypt empty failed");
        assert!(decrypted.is_empty());
    }

    #[test]
    fn test_wrong_key_rejection() {
        let key_a = [0xAAu8; 32];
        let key_b = [0xBBu8; 32];
        let nonce = [0x01u8; NONCE_LEN];
        let envelope = CryptoEnvelope::encrypt(&key_a, &nonce, b"secret data").unwrap();

        let err = CryptoEnvelope::decrypt(&key_b, &envelope).unwrap_err();
        assert!(
            matches!(err, StorageError::AuthenticationError(_) | StorageError::AuthenticationFailed(_)),
            "Wrong key must fail authentication"
        );
    }

    #[test]
    fn test_tampered_ciphertext_rejection() {
        let key = [0x12u8; 32];
        let nonce = [0x34u8; NONCE_LEN];
        let mut envelope = CryptoEnvelope::encrypt(&key, &nonce, b"important payload").unwrap();

        envelope[17] ^= 0x01; // Tamper with first byte of ciphertext
        let err = CryptoEnvelope::decrypt(&key, &envelope).unwrap_err();
        assert!(
            matches!(err, StorageError::AuthenticationError(_) | StorageError::AuthenticationFailed(_)),
            "Tampered ciphertext must fail authentication"
        );
    }

    #[test]
    fn test_tampered_nonce_rejection() {
        let key = [0x12u8; 32];
        let nonce = [0x34u8; NONCE_LEN];
        let mut envelope = CryptoEnvelope::encrypt(&key, &nonce, b"important payload").unwrap();

        envelope[6] ^= 0xFF; // Tamper with nonce byte
        let err = CryptoEnvelope::decrypt(&key, &envelope).unwrap_err();
        assert!(
            matches!(err, StorageError::AuthenticationError(_) | StorageError::AuthenticationFailed(_)),
            "Tampered nonce must fail authentication"
        );
    }

    #[test]
    fn test_tampered_auth_tag_rejection() {
        let key = [0x12u8; 32];
        let nonce = [0x34u8; NONCE_LEN];
        let mut envelope = CryptoEnvelope::encrypt(&key, &nonce, b"important payload").unwrap();

        let last_idx = envelope.len() - 1;
        envelope[last_idx] ^= 0x80; // Tamper with tag byte
        let err = CryptoEnvelope::decrypt(&key, &envelope).unwrap_err();
        assert!(
            matches!(err, StorageError::AuthenticationError(_) | StorageError::AuthenticationFailed(_)),
            "Tampered auth tag must fail authentication"
        );
    }

    #[test]
    fn test_invalid_magic_bytes() {
        let key = [0x12u8; 32];
        let nonce = [0x34u8; NONCE_LEN];
        let mut envelope = CryptoEnvelope::encrypt(&key, &nonce, b"payload").unwrap();

        envelope[0] = b'X';
        let err = CryptoEnvelope::decrypt(&key, &envelope).unwrap_err();
        assert!(matches!(err, StorageError::InvalidMagic));
    }

    #[test]
    fn test_unsupported_version() {
        let key = [0x12u8; 32];
        let nonce = [0x34u8; NONCE_LEN];
        let mut envelope = CryptoEnvelope::encrypt(&key, &nonce, b"payload").unwrap();

        envelope[4] = 2; // Version 2
        let err = CryptoEnvelope::decrypt(&key, &envelope).unwrap_err();
        assert!(matches!(err, StorageError::UnsupportedVersion(2)));
    }

    #[test]
    fn test_raw_disk_byte_opacity_and_ledger_validation() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("secure_ledger.db");
        let svid = Svid::new("spiffe://aura.local/agent/task-runner-01");
        let key = derive_storage_key(&svid);

        let event1 = TaskEvent::new("canary-task-999", TaskState::Pending, 1728690000)
            .with_transition_reason("Task created by orchestrator");

        let mut meta = HashMap::new();
        meta.insert("secret_key".to_string(), "SECRET-KEY-sk-9988776655".to_string());
        meta.insert("endpoint".to_string(), "https://internal.vault/secrets".to_string());

        let event2 = TaskEvent::new("canary-task-999", TaskState::Running, 1728690005)
            .with_json_metadata(&meta)
            .with_transition_reason("Dispatched to Firecracker microVM");

        let event3 = TaskEvent::new("canary-task-999", TaskState::Completed, 1728690020)
            .with_metadata("{\"exit_code\": 0}")
            .with_transition_reason("Execution succeeded with zero errors");

        // 1. Initialize TaskLedger and append events
        {
            let ledger = TaskLedger::open(&db_path, key).expect("open ledger failed");
            ledger.append_event(&event1).expect("append event 1 failed");
            ledger.append_event(&event2).expect("append event 2 failed");
            ledger.append_event(&event3).expect("append event 3 failed");
        }

        // 2. Read raw disk bytes and perform cryptographic opacity verification
        let raw_bytes = std::fs::read(&db_path).expect("read raw db bytes failed");
        assert!(raw_bytes.len() >= MIN_ENVELOPE_LEN);

        // Assert b"AURA\x01" header present
        assert_eq!(&raw_bytes[0..4], b"AURA", "Magic must be b'AURA'");
        assert_eq!(raw_bytes[4], 1, "Version must be 1");
        assert_eq!(&raw_bytes[0..5], b"AURA\x01", "Header must match b'AURA\\x01'");

        // Assert b"SQLite format 3\0" is NOT present anywhere on disk
        assert!(
            !contains_subslice(&raw_bytes, b"SQLite format 3\0"),
            "CRITICAL LEAK: SQLite format 3 header found on raw disk!"
        );
        assert!(
            !contains_subslice(&raw_bytes, b"SQLite format 3"),
            "CRITICAL LEAK: SQLite format 3 found on raw disk!"
        );

        // Assert table names (task_events) are NOT present in raw bytes
        assert!(
            !contains_subslice(&raw_bytes, b"task_events"),
            "CRITICAL LEAK: Table name 'task_events' found in plaintext on disk!"
        );
        assert!(
            !contains_subslice(&raw_bytes, b"CREATE TABLE"),
            "CRITICAL LEAK: DDL statement 'CREATE TABLE' found in plaintext on disk!"
        );

        // Assert task strings and secret payloads are NOT present in raw bytes
        assert!(
            !contains_subslice(&raw_bytes, b"canary-task-999"),
            "CRITICAL LEAK: Task ID 'canary-task-999' found in plaintext on disk!"
        );
        assert!(
            !contains_subslice(&raw_bytes, b"SECRET-KEY-sk-9988776655"),
            "CRITICAL LEAK: Sensitive credential string found in plaintext on disk!"
        );
        assert!(
            !contains_subslice(&raw_bytes, b"Dispatched to Firecracker microVM"),
            "CRITICAL LEAK: Transition reason found in plaintext on disk!"
        );

        // Lossy string inspection
        let text_dump = String::from_utf8_lossy(&raw_bytes);
        assert!(!text_dump.contains("canary-task-999"));
        assert!(!text_dump.contains("SECRET-KEY"));
        assert!(!text_dump.contains("task_events"));

        // 3. Re-open with authentic identity key: verify full data fidelity
        {
            let reopened_ledger = TaskLedger::open_with_svid(&db_path, &svid).expect("reopen failed");
            let events = reopened_ledger.get_events("canary-task-999").expect("get_events failed");
            assert_eq!(events.len(), 3);
            assert_eq!(events[0], event1);
            assert_eq!(events[1], event2);
            assert_eq!(events[2], event3);
        }

        // 4. Re-open with unauthorized key: must fail authentication
        {
            let rogue_svid = Svid::new("spiffe://aura.local/agent/rogue-attacker");
            let rogue_result = TaskLedger::open_with_svid(&db_path, &rogue_svid);
            assert!(rogue_result.is_err(), "Opening with rogue SVID must fail");
        }

        // 5. Tampering with file on disk: must fail authentication
        {
            let mut tampered_bytes = raw_bytes.clone();
            tampered_bytes[25] ^= 0x55;
            let tampered_path = dir.path().join("tampered.db");
            std::fs::write(&tampered_path, tampered_bytes).unwrap();

            let tamper_result = TaskLedger::open(&tampered_path, key);
            assert!(tamper_result.is_err(), "Opening tampered file must fail");
        }
    }

    #[test]
    fn test_encrypted_database_crud_operations() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("crud.db");
        let key = [0x99u8; 32];

        // Create table and insert
        {
            let db = EncryptedDatabase::open(&db_path, key).unwrap();
            db.execute_batch(
                "CREATE TABLE configs (key TEXT PRIMARY KEY, val TEXT);
                 INSERT INTO configs VALUES ('theme', 'dark');
                 INSERT INTO configs VALUES ('timeout', '30');",
            )
            .unwrap();
            db.flush().unwrap();
        }

        // Reopen and query
        {
            let db = EncryptedDatabase::open(&db_path, key).unwrap();
            let val: String = db
                .query_row("SELECT val FROM configs WHERE key = ?1", &[&"theme"], |r| r.get(0))
                .unwrap();
            assert_eq!(val, "dark");

            let rows: Vec<(String, String)> = db
                .query("SELECT key, val FROM configs ORDER BY key", [], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })
                .unwrap();
            assert_eq!(rows.len(), 2);
            assert_eq!(rows[0], ("theme".to_string(), "dark".to_string()));
            assert_eq!(rows[1], ("timeout".to_string(), "30".to_string()));
        }
    }

    #[test]
    fn test_encrypted_database_debug_and_generic_query_row() {
        let key = [0x5au8; 32];
        let db = EncryptedDatabase::open_in_memory(key).unwrap();

        // 1. Verify Debug formatting and key redaction
        let debug_str = format!("{:?}", db);
        assert!(debug_str.contains("EncryptedDatabase"), "Must format as EncryptedDatabase");
        assert!(debug_str.contains("[REDACTED-STORAGE-KEY]"), "Must redact encryption key");
        assert!(!debug_str.contains("5a"), "Raw key bytes must not appear in debug output");

        // 2. Verify unwrap_err works via Debug bound on EncryptedDatabase
        let bad_bytes = b"CORRUPTED_NOT_ENVELOPE";
        let dir = tempdir().unwrap();
        let corrupted_file = dir.path().join("corrupted.db");
        std::fs::write(&corrupted_file, bad_bytes).unwrap();
        let err: StorageError = EncryptedDatabase::open(&corrupted_file, key).unwrap_err();
        assert!(matches!(err, StorageError::EnvelopeTooShort | StorageError::InvalidMagic));

        // 3. Verify query_row accepts [] directly without type coercion errors
        db.execute_batch("CREATE TABLE kv (k TEXT PRIMARY KEY, v INT); INSERT INTO kv VALUES ('a', 42);")
            .unwrap();
        let val: i32 = db.query_row("SELECT v FROM kv WHERE k = 'a'", [], |r| r.get(0)).unwrap();
        assert_eq!(val, 42);

        // 4. Verify query_row also accepts rusqlite slice &[&dyn ToSql]
        let val2: i32 = db
            .query_row("SELECT v FROM kv WHERE k = ?1", &[&"a"], |r| r.get(0))
            .unwrap();
        assert_eq!(val2, 42);
    }

    #[test]
    fn test_task_ledger_debug() {
        let key = [0x33u8; 32];
        let ledger = TaskLedger::in_memory(key).unwrap();

        let debug_str = format!("{:?}", ledger);
        assert!(debug_str.contains("TaskLedger"), "Must format as TaskLedger");
        assert!(debug_str.contains("EncryptedDatabase"), "Must format inner db");
        assert!(debug_str.contains("[REDACTED-STORAGE-KEY]"), "Must redact storage key");

        // Verify unwrap_err works on TaskLedger
        let dir = tempdir().unwrap();
        let bad_file = dir.path().join("bad_ledger.db");
        std::fs::write(&bad_file, b"TOO_SHORT").unwrap();
        let err: StorageError = TaskLedger::open(&bad_file, key).unwrap_err();
        assert!(matches!(err, StorageError::EnvelopeTooShort | StorageError::InvalidMagic));
    }
}

