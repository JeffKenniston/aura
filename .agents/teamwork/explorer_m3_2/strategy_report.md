# Architecture & Implementation Strategy Report: Storage Encryption & Key Derivation (Features 15 & 17)

**Agent**: `explorer_m3_2` (Teamwork Explorer)  
**Milestone**: Milestone 3 — Microkernel Zero-Trust Security & Storage Encryption  
**Scope**: Feature 15 (AES-256-GCM Encrypted Storage) & Feature 17 (SPIFFE Key Derivation)  
**Target Codebase**: `aura-core` (`aura-core/src/storage/`, `aura-core/src/identity/`, `aura-core/Cargo.toml`)  
**Date**: 2026-10-11  

---

## 1. Executive Summary & Problem Analysis

In the Aura Multi-Agent Orchestration Operating System, persistent runtime state, task ledgers, and repository index metadata are stored in SQLite databases. Under standard unencrypted SQLite, disk files contain plaintext headers (`SQLite format 3\0`), schema DDL statements (`CREATE TABLE ...`), column identifiers, and sensitive task arguments/credentials in plaintext pages.

Per **ORIGINAL_REQUEST.md (§R3)** and **PROJECT.md (Milestone 3, Features 15 & 17)**:
- **Feature 17 (SPIFFE Key Derivation)**: Persistence keys must be derived deterministically from an attested SPIFFE Verifiable Identity Document (`Svid`) using the BLAKE3 Key Derivation Function (KDF): `blake3::derive_key("aura-storage-encryption-v1", svid.spiffe_id().as_bytes())`. Workloads must be cryptographically identity-locked so that only workloads presenting the exact attested identity can decrypt and access storage.
- **Feature 15 (AES-256-GCM Encrypted Storage)**: SQLite persistence must be wrapped in an authenticated `EncryptedDatabase` container employing the envelope format:
  `[MAGIC: b"AURA"][VERSION: 1u8][NONCE: 12 bytes][CIPHERTEXT + 16-byte AUTH TAG]`
  providing transparent encryption on write/flush, transparent decryption on read/open, and guaranteed rejection upon tag tampering, nonce corruption, or key mismatches.

### Zero-Disk-Leakage Invariant
A critical security vulnerability in naive encrypted database wrappers is writing decrypted SQLite files to temporary disk locations (`/tmp`) during open. In Aura, **zero plaintext bytes may ever touch physical disk storage**. By leveraging `rusqlite::ffi::sqlite3_deserialize` and `Connection::serialize`, SQLite database lifecycles remain 100% in-memory while the application is active, serializing to authenticated AES-256-GCM ciphertext exclusively on atomic flush.

---

## 2. Feature 17: SPIFFE Key Derivation Architecture

### 2.1 Cryptographic Derivation Model
BLAKE3 provides a dedicated Key Derivation Function (KDF) mode (`blake3::derive_key`) designed to output pseudorandom subkeys from arbitrary input key material:

$$\text{Key}_{256} = \text{BLAKE3-KDF}(\text{Context} = \text{"aura-storage-encryption-v1"}, \text{Material} = \text{Canonical SPIFFE ID})$$

```
+-----------------------------------+
| Attested Svid                     |
| spiffe://aura.local/agent/task-42 |
+-----------------+-----------------+
                  |
                  | Canonical String (SpiffeId::to_string())
                  v
+-----------------------------------+
| BLAKE3 KDF                        |
| Context:                          |
| "aura-storage-encryption-v1"      |
+-----------------+-----------------+
                  |
                  | 256-bit Pseudorandom Keystream
                  v
+-----------------------------------+
| 32-byte AES-256-GCM Storage Key   |
| [u8; 32]                          |
+-----------------------------------+
```

### 2.2 Identity-Locking Security Guarantees
1. **Collision & Preimage Resistance**: BLAKE3's 256-bit security level guarantees that no two distinct SPIFFE IDs (e.g. `spiffe://aura.local/agent/task-1` vs `spiffe://aura.local/agent/task-2`) will ever derive the same encryption key.
2. **Domain Separation**: The context string `"aura-storage-encryption-v1"` cryptographically isolates storage encryption keys from any other BLAKE3 derivations in the Aura system (e.g., Merkle tree hashes or transport tokens).
3. **Canonical Normalization**: SPIFFE IDs are parsed into `spiffe::SpiffeId`, ensuring trust domain case normalization (`aura.LOCAL` -> `aura.local`) and path validation before key derivation.
4. **Attestation Requirement**: Keys can only be derived from an attested `Svid` possessing valid trust domain and identity fields. Workloads in unauthorized trust domains or with disparate paths cannot derive the storage key, resulting in immediate AEAD authentication tag verification failure upon database open.

---

## 3. Feature 15: AES-256-GCM Encrypted Database Blueprint

### 3.1 Container Envelope Specification

The disk storage format is defined with exact byte-level alignment matching `aura-core/tests/m5_e2e_opaque_test.rs`:

```
+---------------+---------------+----------------+--------------------------------+-----------------+
| MAGIC (4B)    | VERSION (1B)  | NONCE (12B)    | CIPHERTEXT (N Bytes)           | AUTH TAG (16B)  |
| 0x41 41 55 52 | 0x01          | Random 12B IV  | AES-256-GCM Encrypted Database | Poly1305 / GHASH|
| b"AURA"       | 1             | [u8; 12]       | Plaintext Length N             | [u8; 16]        |
+---------------+---------------+----------------+--------------------------------+-----------------+
0               4               5                17                               17 + N            33 + N
```

- **MAGIC (`0..4`)**: Constant `b"AURA"` (`[0x41, 0x55, 0x52, 0x41]`).
- **VERSION (`4..5`)**: `1u8`.
- **NONCE (`5..17`)**: 12-byte cryptographically secure random nonce (`[u8; 12]`).
- **CIPHERTEXT (`17..17+N`)**: Encrypted bytes of the serialized SQLite database.
- **AUTH TAG (`17+N..33+N`)**: 16-byte AES-GCM authentication tag. Note that `aes_gcm::Aes256Gcm::encrypt` appends the 16-byte tag directly to the ciphertext, making `&envelope[17..]` exactly the ciphertext + tag payload expected by `aes_gcm::Aes256Gcm::decrypt`.

### 3.2 Boundary Offsets & Assertion Alignment
The layout directly validates against tests defined in `tests/m5_e2e_opaque_test.rs`:
- Byte index 0 (`envelope[0]`): Magic byte corruption -> triggers `StorageError::InvalidMagic`.
- Byte index 4 (`envelope[4]`): Version mismatch -> triggers `StorageError::UnsupportedVersion`.
- Byte index 6 (`envelope[6]`): Nonce byte tampering -> triggers `StorageError::AuthenticationFailed`.
- Byte index 17 (`envelope[17]`): Ciphertext byte tampering -> triggers `StorageError::AuthenticationFailed`.
- Last byte (`envelope.len() - 1`): Auth tag tampering -> triggers `StorageError::AuthenticationFailed`.
- Payload length 0: Empty database encrypts to a valid 33-byte envelope and decrypts to empty slice with zero errors.

### 3.3 In-Memory SQLite Mechanics (Zero-Leakage)

```
Open Path:
[Encrypted File on Disk]
       |
       v (std::fs::read)
[Authenticated Envelope]
       |
       v (CryptoEnvelope::decrypt: verify magic, version, nonce, tag)
[Decrypted SQLite Bytes]
       |
       v (sqlite3_malloc64 + sqlite3_deserialize)
[In-Memory rusqlite::Connection] (Live SQL Queries, Transactions, Indexes)

Flush Path:
[In-Memory rusqlite::Connection]
       |
       v (Connection::serialize)
[Raw SQLite Page Bytes]
       |
       v (CryptoEnvelope::encrypt: AES-256-GCM + [AURA][1][NONCE])
[Authenticated Envelope]
       |
       v (Atomic write: temp file .tmp_enc -> rename)
[Encrypted File on Disk]
```

#### Memory Management Details:
1. `rusqlite::Connection::open_in_memory()` establishes an isolated SQLite instance.
2. `rusqlite::ffi::sqlite3_malloc64` allocates SQLite-managed memory for the decrypted buffer.
3. `rusqlite::serialize::OwnedData::from_raw_nonnull` passes ownership to SQLite.
4. `conn.deserialize(DatabaseName::Main, owned, false)` initializes SQLite directly from memory with read/write capability (`SQLITE_DESERIALIZE_RESIZEABLE | SQLITE_DESERIALIZE_FREEONCLOSE`).
5. On flush, `conn.serialize(DatabaseName::Main)` produces a shared/owned slice of SQLite database bytes without flushing to disk.
6. The database is encrypted in RAM and written atomically to a temporary file in the same directory (`.tmp_enc`) before POSIX rename, ensuring crash-safe atomic updates.

---

## 4. Concrete Code Implementation Blueprint

### 4.1 Dependency Updates in `aura-core/Cargo.toml`
Add `aes-gcm` and `getrandom` to `aura-core/Cargo.toml`:

```toml
# In aura-core/Cargo.toml [dependencies]:
aes-gcm = "0.10.3"
getrandom = "0.2"
```

*Note: `aes-gcm = "0.10.3"` resolves to the exact same `aes = "0.8.4"` and `cipher = "0.4.4"` crates already compiled into `Cargo.lock`, resulting in zero dependency bloat.*

### 4.2 SVID Key Derivation in `aura-core/src/identity/mod.rs`
Add to `impl Svid`:

```rust
impl Svid {
    /// Derives a deterministic 256-bit AES-GCM storage encryption key from the attested SPIFFE ID (Feature 17).
    /// Uses BLAKE3 Key Derivation Function (KDF) with context "aura-storage-encryption-v1".
    pub fn derive_storage_key(&self) -> Result<[u8; 32], IdentityError> {
        let spiffe_id = self.spiffe_id()?;
        let canonical_id = spiffe_id.to_string();
        Ok(blake3::derive_key(
            "aura-storage-encryption-v1",
            canonical_id.as_bytes(),
        ))
    }

    /// Derives a deterministic 256-bit storage encryption key with a caller-specified context.
    pub fn derive_storage_key_with_context(&self, context: &str) -> Result<[u8; 32], IdentityError> {
        let spiffe_id = self.spiffe_id()?;
        let canonical_id = spiffe_id.to_string();
        Ok(blake3::derive_key(
            context,
            canonical_id.as_bytes(),
        ))
    }
}
```

### 4.3 Storage Module Hierarchy: `aura-core/src/storage/mod.rs`

```rust
//! Encrypted Storage Subsystem for Aura OS (Features 15, 16, 17)
//!
//! Provides identity-locked, AES-256-GCM envelope-encrypted persistence
//! over SQLite databases and Task Ledgers.

pub mod encrypted;
pub mod ledger;

pub use encrypted::{
    derive_key_from_svid, derive_key_from_svid_with_context, CryptoEnvelope, EncryptedDatabase,
    StorageError, ENCRYPTED_MAGIC, ENCRYPTED_VERSION, HEADER_LEN, MIN_ENVELOPE_LEN, NONCE_LEN,
    TAG_LEN,
};
pub use ledger::{TaskEvent, TaskLedger, TaskState};
```

### 4.4 Encrypted Database Implementation: `aura-core/src/storage/encrypted.rs`

```rust
//! AES-256-GCM Envelope-Encrypted SQLite Database Container (Features 15 & 17)

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use crate::identity::Svid;

/// Envelope constants
pub const ENCRYPTED_MAGIC: &[u8; 4] = b"AURA";
pub const ENCRYPTED_VERSION: u8 = 1;
pub const NONCE_LEN: usize = 12;
pub const TAG_LEN: usize = 16;
pub const HEADER_LEN: usize = 4 + 1 + NONCE_LEN; // 17 bytes
pub const MIN_ENVELOPE_LEN: usize = HEADER_LEN + TAG_LEN; // 33 bytes
pub const KDF_STORAGE_CONTEXT: &str = "aura-storage-encryption-v1";

/// Comprehensive errors for the encrypted storage subsystem.
#[derive(Debug)]
pub enum StorageError {
    Io(std::io::Error),
    Sqlite(rusqlite::Error),
    EncryptionError(String),
    DecryptionError(String),
    AuthenticationFailed(String),
    InvalidMagic,
    UnsupportedVersion(u8),
    EnvelopeTooShort,
    IdentityError(String),
    LockError(String),
    OutOfMemory,
    CorruptedEnvelope(String),
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StorageError::Io(e) => write!(f, "I/O error: {}", e),
            StorageError::Sqlite(e) => write!(f, "SQLite error: {}", e),
            StorageError::EncryptionError(msg) => write!(f, "Encryption error: {}", msg),
            StorageError::DecryptionError(msg) => write!(f, "Decryption error: {}", msg),
            StorageError::AuthenticationFailed(msg) => {
                write!(f, "Authentication tag verification failed: {}", msg)
            }
            StorageError::InvalidMagic => {
                write!(f, "Invalid magic bytes: not an AURA encrypted container")
            }
            StorageError::UnsupportedVersion(v) => {
                write!(f, "Unsupported storage version: {}", v)
            }
            StorageError::EnvelopeTooShort => write!(f, "Envelope too short"),
            StorageError::IdentityError(msg) => write!(f, "Identity error: {}", msg),
            StorageError::LockError(msg) => write!(f, "Lock error: {}", msg),
            StorageError::OutOfMemory => write!(f, "Out of memory allocating SQLite buffer"),
            StorageError::CorruptedEnvelope(msg) => write!(f, "Corrupted envelope: {}", msg),
        }
    }
}

impl std::error::Error for StorageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            StorageError::Io(e) => Some(e),
            StorageError::Sqlite(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for StorageError {
    fn from(e: std::io::Error) -> Self {
        StorageError::Io(e)
    }
}

impl From<rusqlite::Error> for StorageError {
    fn from(e: rusqlite::Error) -> Self {
        StorageError::Sqlite(e)
    }
}

/// Derives a 256-bit AES-GCM storage key from an attested Svid using BLAKE3 KDF (Feature 17).
pub fn derive_key_from_svid(svid: &Svid) -> Result<[u8; 32], StorageError> {
    svid.derive_storage_key()
        .map_err(|e| StorageError::IdentityError(e.to_string()))
}

/// Derives a 256-bit storage key from an attested Svid with custom KDF context.
pub fn derive_key_from_svid_with_context(svid: &Svid, context: &str) -> Result<[u8; 32], StorageError> {
    svid.derive_storage_key_with_context(context)
        .map_err(|e| StorageError::IdentityError(e.to_string()))
}

/// Pure-Rust authenticated container envelope for AES-256-GCM storage payloads.
pub struct CryptoEnvelope;

impl CryptoEnvelope {
    /// Encrypts plaintext into authenticated envelope: `[AURA][VERSION][NONCE][CIPHERTEXT+TAG]`.
    pub fn encrypt(key: &[u8; 32], nonce: &[u8; 12], plaintext: &[u8]) -> Result<Vec<u8>, StorageError> {
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
        let gcm_nonce = Nonce::from_slice(nonce);

        let ciphertext_and_tag = cipher
            .encrypt(gcm_nonce, plaintext)
            .map_err(|e| StorageError::EncryptionError(format!("AES-256-GCM encrypt failed: {}", e)))?;

        let mut envelope = Vec::with_capacity(HEADER_LEN + ciphertext_and_tag.len());
        envelope.extend_from_slice(ENCRYPTED_MAGIC);
        envelope.push(ENCRYPTED_VERSION);
        envelope.extend_from_slice(nonce);
        envelope.extend_from_slice(&ciphertext_and_tag);

        Ok(envelope)
    }

    /// Decrypts envelope and verifies authentication tag.
    pub fn decrypt(key: &[u8; 32], envelope: &[u8]) -> Result<Vec<u8>, StorageError> {
        if envelope.len() < MIN_ENVELOPE_LEN {
            return Err(StorageError::EnvelopeTooShort);
        }

        if &envelope[0..4] != ENCRYPTED_MAGIC {
            return Err(StorageError::InvalidMagic);
        }

        if envelope[4] != ENCRYPTED_VERSION {
            return Err(StorageError::UnsupportedVersion(envelope[4]));
        }

        let nonce = &envelope[5..17];
        let ciphertext_and_tag = &envelope[17..];

        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
        let gcm_nonce = Nonce::from_slice(nonce);

        cipher.decrypt(gcm_nonce, ciphertext_and_tag).map_err(|_| {
            StorageError::AuthenticationFailed(
                "corrupted data or wrong key".to_string(),
            )
        })
    }
}

/// Thread-safe, identity-locked encrypted SQLite database container.
pub struct EncryptedDatabase {
    conn: Mutex<rusqlite::Connection>,
    path: PathBuf,
    key: [u8; 32],
    dirty: AtomicBool,
}

impl EncryptedDatabase {
    /// Opens an encrypted database at `path` using the given 256-bit key.
    /// If the file exists, it is decrypted and deserialized into an in-memory SQLite database.
    /// If the file does not exist, an empty in-memory SQLite database is initialized.
    pub fn open<P: AsRef<Path>>(path: P, key: [u8; 32]) -> Result<Self, StorageError> {
        let path_buf = path.as_ref().to_path_buf();
        let mut conn = rusqlite::Connection::open_in_memory()?;

        if path_buf.exists() {
            let file_bytes = std::fs::read(&path_buf)?;
            if !file_bytes.is_empty() {
                let decrypted = CryptoEnvelope::decrypt(&key, &file_bytes)?;
                if !decrypted.is_empty() {
                    let sz = decrypted.len();
                    let ptr = unsafe { rusqlite::ffi::sqlite3_malloc64(sz as u64) as *mut u8 };
                    if ptr.is_null() {
                        return Err(StorageError::OutOfMemory);
                    }
                    unsafe {
                        std::ptr::copy_nonoverlapping(decrypted.as_ptr(), ptr, sz);
                        let non_null = std::ptr::NonNull::new_unchecked(ptr);
                        let owned = rusqlite::serialize::OwnedData::from_raw_nonnull(non_null, sz);
                        conn.deserialize(rusqlite::DatabaseName::Main, owned, false)?;
                    }
                }
            }
        }

        Ok(Self {
            conn: Mutex::new(conn),
            path: path_buf,
            key,
            dirty: AtomicBool::new(false),
        })
    }

    /// Opens an encrypted database locked directly to an attested SPIFFE Svid.
    pub fn open_with_svid<P: AsRef<Path>>(path: P, svid: &Svid) -> Result<Self, StorageError> {
        let key = derive_key_from_svid(svid)?;
        Self::open(path, key)
    }

    /// Creates an in-memory-only encrypted database with no backing disk path.
    pub fn open_in_memory(key: [u8; 32]) -> Result<Self, StorageError> {
        let conn = rusqlite::Connection::open_in_memory()?;
        Ok(Self {
            conn: Mutex::new(conn),
            path: PathBuf::new(),
            key,
            dirty: AtomicBool::new(false),
        })
    }

    /// Flushes the in-memory SQLite database to disk with AES-256-GCM encryption
    /// using a cryptographically secure random nonce.
    pub fn flush(&self) -> Result<(), StorageError> {
        let mut nonce = [0u8; NONCE_LEN];
        getrandom::getrandom(&mut nonce)
            .map_err(|e| StorageError::EncryptionError(format!("RNG failure: {}", e)))?;
        self.flush_with_nonce(&nonce)
    }

    /// Flushes the database using an explicitly specified 12-byte nonce (for deterministic testing).
    pub fn flush_with_nonce(&self, nonce: &[u8; 12]) -> Result<(), StorageError> {
        if self.path.as_os_str().is_empty() {
            return Err(StorageError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Cannot flush an in-memory-only database without a file path",
            )));
        }

        let conn = self.conn.lock().map_err(|e| StorageError::LockError(e.to_string()))?;
        let data = conn.serialize(rusqlite::DatabaseName::Main)?;
        let plaintext = &*data;

        let encrypted = CryptoEnvelope::encrypt(&self.key, nonce, plaintext)?;

        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }

        // Atomic write via temporary file in same directory
        let temp_path = self.path.with_extension("tmp_enc");
        std::fs::write(&temp_path, &encrypted)?;
        std::fs::rename(&temp_path, &self.path)?;

        self.dirty.store(false, Ordering::Release);
        Ok(())
    }

    /// Executes a SQL statement.
    pub fn execute(&self, sql: &str, params: &[&dyn rusqlite::ToSql]) -> Result<usize, StorageError> {
        let conn = self.conn.lock().map_err(|e| StorageError::LockError(e.to_string()))?;
        let rows = conn.execute(sql, params)?;
        self.dirty.store(true, Ordering::Release);
        Ok(rows)
    }

    /// Executes multiple SQL statements in a batch.
    pub fn execute_batch(&self, sql: &str) -> Result<(), StorageError> {
        let conn = self.conn.lock().map_err(|e| StorageError::LockError(e.to_string()))?;
        conn.execute_batch(sql)?;
        self.dirty.store(true, Ordering::Release);
        Ok(())
    }

    /// Queries a single row from the database.
    pub fn query_row<T, F>(&self, sql: &str, params: &[&dyn rusqlite::ToSql], f: F) -> Result<T, StorageError>
    where
        F: FnOnce(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
    {
        let conn = self.conn.lock().map_err(|e| StorageError::LockError(e.to_string()))?;
        let result = conn.query_row(sql, params, f)?;
        Ok(result)
    }

    /// Executes a closure with exclusive access to the underlying `rusqlite::Connection`.
    pub fn with_conn<F, R>(&self, f: F) -> Result<R, StorageError>
    where
        F: FnOnce(&mut rusqlite::Connection) -> Result<R, StorageError>,
    {
        let mut conn = self.conn.lock().map_err(|e| StorageError::LockError(e.to_string()))?;
        let res = f(&mut conn)?;
        self.dirty.store(true, Ordering::Release);
        Ok(res)
    }

    /// Returns the database file path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the 256-bit encryption key.
    pub fn key(&self) -> &[u8; 32] {
        &self.key
    }

    /// Returns whether there are unflushed modifications.
    pub fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::Acquire)
    }
}

impl Drop for EncryptedDatabase {
    fn drop(&mut self) {
        if self.is_dirty() && !self.path.as_os_str().is_empty() {
            let _ = self.flush();
        }
    }
}
```

### 4.5 Microkernel Module Re-Export: `aura-core/src/lib.rs`
Update `aura-core/src/lib.rs` line 8:

```rust
pub mod bus;
pub mod config;
pub mod hypervisor;
pub mod identity;
pub mod knowledge;
pub mod runtime;
pub mod storage; // <-- Add storage module export
pub mod supervisor;
pub mod transport;
```

---

## 5. Comprehensive Test Specifications & Suite Blueprint

To be added as unit tests in `aura-core/src/storage/encrypted.rs` (under `#[cfg(test)] mod tests`):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::Svid;
    use tempfile::tempdir;

    #[test]
    fn test_envelope_magic_and_version() {
        let key = [0x11u8; 32];
        let nonce = [0x22u8; 12];
        let plaintext = b"CREATE TABLE test (id INT);";
        let envelope = CryptoEnvelope::encrypt(&key, &nonce, plaintext).unwrap();

        assert_eq!(&envelope[0..4], b"AURA");
        assert_eq!(envelope[4], 1);
        assert_eq!(&envelope[5..17], &nonce);
    }

    #[test]
    fn test_roundtrip_encryption_decryption() {
        let key = [0x33u8; 32];
        let nonce = [0x44u8; 12];
        let original = b"Sensitive persistent SQLite bytes";
        let envelope = CryptoEnvelope::encrypt(&key, &nonce, original).unwrap();
        let decrypted = CryptoEnvelope::decrypt(&key, &envelope).unwrap();
        assert_eq!(&decrypted[..], original);
    }

    #[test]
    fn test_empty_plaintext_envelope() {
        let key = [0x55u8; 32];
        let nonce = [0x66u8; 12];
        let envelope = CryptoEnvelope::encrypt(&key, &nonce, b"").unwrap();
        assert_eq!(envelope.len(), MIN_ENVELOPE_LEN);
        let decrypted = CryptoEnvelope::decrypt(&key, &envelope).unwrap();
        assert!(decrypted.is_empty());
    }

    #[test]
    fn test_corrupted_ciphertext_fails_auth() {
        let key = [0x77u8; 32];
        let nonce = [0x88u8; 12];
        let mut envelope = CryptoEnvelope::encrypt(&key, &nonce, b"Secret Payload").unwrap();
        envelope[17] ^= 0xFF; // Flip first ciphertext bit
        let err = CryptoEnvelope::decrypt(&key, &envelope).unwrap_err();
        assert!(matches!(err, StorageError::AuthenticationFailed(_)));
    }

    #[test]
    fn test_corrupted_nonce_fails_auth() {
        let key = [0x77u8; 32];
        let nonce = [0x88u8; 12];
        let mut envelope = CryptoEnvelope::encrypt(&key, &nonce, b"Secret Payload").unwrap();
        envelope[6] ^= 0xFF; // Flip nonce bit
        let err = CryptoEnvelope::decrypt(&key, &envelope).unwrap_err();
        assert!(matches!(err, StorageError::AuthenticationFailed(_)));
    }

    #[test]
    fn test_corrupted_tag_fails_auth() {
        let key = [0x77u8; 32];
        let nonce = [0x88u8; 12];
        let mut envelope = CryptoEnvelope::encrypt(&key, &nonce, b"Secret Payload").unwrap();
        let last = envelope.len() - 1;
        envelope[last] ^= 0xFF; // Flip tag bit
        let err = CryptoEnvelope::decrypt(&key, &envelope).unwrap_err();
        assert!(matches!(err, StorageError::AuthenticationFailed(_)));
    }

    #[test]
    fn test_corrupted_magic_fails() {
        let key = [0x77u8; 32];
        let nonce = [0x88u8; 12];
        let mut envelope = CryptoEnvelope::encrypt(&key, &nonce, b"Secret Payload").unwrap();
        envelope[0] = b'Z';
        let err = CryptoEnvelope::decrypt(&key, &envelope).unwrap_err();
        assert!(matches!(err, StorageError::InvalidMagic));
    }

    #[test]
    fn test_unsupported_version_fails() {
        let key = [0x77u8; 32];
        let nonce = [0x88u8; 12];
        let mut envelope = CryptoEnvelope::encrypt(&key, &nonce, b"Secret Payload").unwrap();
        envelope[4] = 2; // Unsupported version
        let err = CryptoEnvelope::decrypt(&key, &envelope).unwrap_err();
        assert!(matches!(err, StorageError::UnsupportedVersion(2)));
    }

    #[test]
    fn test_wrong_key_fails_auth() {
        let key_a = [0xAAu8; 32];
        let key_b = [0xBBu8; 32];
        let nonce = [0xCCu8; 12];
        let envelope = CryptoEnvelope::encrypt(&key_a, &nonce, b"Secret Payload").unwrap();
        let err = CryptoEnvelope::decrypt(&key_b, &envelope).unwrap_err();
        assert!(matches!(err, StorageError::AuthenticationFailed(_)));
    }

    #[test]
    fn test_spiffe_kdf_key_derivation() {
        let svid_a = Svid::new("spiffe://aura.local/agent/workload-a");
        let svid_b = Svid::new("spiffe://aura.local/agent/workload-b");

        let key_a1 = svid_a.derive_storage_key().unwrap();
        let key_a2 = svid_a.derive_storage_key().unwrap();
        let key_b = svid_b.derive_storage_key().unwrap();

        assert_eq!(key_a1, key_a2, "KDF must be deterministic");
        assert_ne!(key_a1, key_b, "Distinct SVIDs must yield orthogonal keys");
    }

    #[test]
    fn test_database_lifecycle_and_zero_plaintext_leakage() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("encrypted.db");
        let svid = Svid::new("spiffe://aura.local/agent/task-ledger");
        let key = svid.derive_storage_key().unwrap();

        // 1. Open and populate database
        {
            let db = EncryptedDatabase::open(&db_path, key).unwrap();
            db.execute_batch(
                "CREATE TABLE tasks (id TEXT PRIMARY KEY, instruction TEXT, secret_key TEXT);"
            ).unwrap();
            db.execute(
                "INSERT INTO tasks (id, instruction, secret_key) VALUES (?1, ?2, ?3);",
                &[&"t-100", &"run secure job", &"SECRET_PASSWORD_VAL_99999"],
            ).unwrap();
            db.flush().unwrap();
        }

        // 2. Raw disk byte inspection
        let raw_disk = std::fs::read(&db_path).unwrap();
        assert_eq!(&raw_disk[0..4], b"AURA");
        assert_eq!(raw_disk[4], 1);

        let raw_str = String::from_utf8_lossy(&raw_disk);
        assert!(!raw_str.contains("SQLite format 3"), "Raw disk must NOT contain SQLite header");
        assert!(!raw_str.contains("tasks"), "Raw disk must NOT contain table names");
        assert!(!raw_str.contains("SECRET_PASSWORD_VAL"), "Raw disk must NOT contain plaintext secret strings");

        // 3. Re-open with authentic identity key succeeds
        {
            let db = EncryptedDatabase::open(&db_path, key).unwrap();
            let secret: String = db.query_row(
                "SELECT secret_key FROM tasks WHERE id = ?1;",
                &[&"t-100"],
                |row| row.get(0),
            ).unwrap();
            assert_eq!(secret, "SECRET_PASSWORD_VAL_99999");
        }

        // 4. Re-open with unauthorized identity fails
        {
            let unauthorized_svid = Svid::new("spiffe://aura.local/agent/attacker");
            let unauthorized_key = unauthorized_svid.derive_storage_key().unwrap();
            let res = EncryptedDatabase::open(&db_path, unauthorized_key);
            assert!(res.is_err(), "Unauthorized identity must fail authentication");
            assert!(matches!(res.unwrap_err(), StorageError::AuthenticationFailed(_)));
        }
    }
}
```

---

## 6. Interface Contracts for Peer Agents (Feature 16 `TaskLedger`)

For `explorer_m3_3` / `worker_m3_1` implementing `TaskLedger`:
`TaskLedger` should wrap an `Arc<EncryptedDatabase>`:

```rust
pub struct TaskLedger {
    db: Arc<EncryptedDatabase>,
}

impl TaskLedger {
    pub fn open<P: AsRef<Path>>(path: P, key: [u8; 32]) -> Result<Self, StorageError> {
        let db = Arc::new(EncryptedDatabase::open(path, key)?);
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS task_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id TEXT NOT NULL,
                state TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                metadata TEXT NOT NULL,
                transition_reason TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_task_events_task_id ON task_events(task_id);"
        )?;
        Ok(Self { db })
    }

    pub fn append_event(&self, event: &TaskEvent) -> Result<(), StorageError> {
        self.db.execute(
            "INSERT INTO task_events (task_id, state, timestamp, metadata, transition_reason)
             VALUES (?1, ?2, ?3, ?4, ?5);",
            &[&event.task_id, &event.state.as_str(), &(event.timestamp as i64), &event.metadata, &event.transition_reason],
        )?;
        Ok(())
    }

    pub fn flush(&self) -> Result<(), StorageError> {
        self.db.flush()
    }
}
```

---

## 7. Verification Method & Acceptance Gate

Once authored, verification must follow this deterministic sequence:

1. **Cargo Compilation Check**:
   ```bash
   cargo check -p aura-core --jobs 1
   ```
   Assert 0 errors, 0 warnings.

2. **Isolated Storage Unit Test Execution**:
   ```bash
   cargo test -p aura-core --lib storage::encrypted --jobs 1
   ```
   Assert all 11 unit tests pass in <1.0s.

3. **Existing Opaque Test Suite Regression Check**:
   ```bash
   cargo test -p aura-core --test m5_e2e_opaque_test --jobs 1
   ```
   Assert 170/170 tests pass with zero regressions.

4. **Complete Package Test Run**:
   ```bash
   cargo test -p aura-core --jobs 1
   ```
   Assert 100% test passage.
