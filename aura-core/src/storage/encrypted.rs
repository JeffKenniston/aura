//! AES-256-GCM Envelope-Encrypted SQLite Database Container (Features 15 & 17)

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use aes_gcm::aead::rand_core::RngCore;
use rusqlite::{Connection, DatabaseName};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::identity::Svid;

/// Envelope magic constant b"AURA"
pub const ENCRYPTED_MAGIC: &[u8; 4] = b"AURA";
/// Envelope version: 1
pub const ENCRYPTED_VERSION: u8 = 1;
/// 12-byte nonce for AES-256-GCM
pub const NONCE_LEN: usize = 12;
/// 16-byte Poly1305 / GHASH authentication tag
pub const TAG_LEN: usize = 16;
/// 17-byte header: 4 (magic) + 1 (version) + 12 (nonce)
pub const HEADER_LEN: usize = 4 + 1 + NONCE_LEN;
/// Minimum envelope length: 17 + 0 (ciphertext) + 16 (tag) = 33 bytes
pub const MIN_ENVELOPE_LEN: usize = HEADER_LEN + TAG_LEN;
/// BLAKE3 KDF context string for storage encryption
pub const KDF_STORAGE_CONTEXT: &str = "aura-storage-encryption-v1";

/// Comprehensive errors for the encrypted storage subsystem.
#[derive(Debug)]
pub enum StorageError {
    AuthenticationError(String),
    AuthenticationFailed(String),
    InvalidMagic,
    UnsupportedVersion(u8),
    EnvelopeTooShort,
    DatabaseError(rusqlite::Error),
    EncryptionError(String),
    DecryptionError(String),
    SerializationError(String),
    Io(std::io::Error),
    IdentityError(String),
    LockError(String),
    CorruptedEnvelope(String),
    Internal(String),
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StorageError::AuthenticationError(msg) => {
                write!(f, "Authentication error: {}", msg)
            }
            StorageError::AuthenticationFailed(msg) => {
                write!(f, "Authentication failed: {}", msg)
            }
            StorageError::InvalidMagic => {
                write!(f, "Invalid magic bytes: not an AURA encrypted container")
            }
            StorageError::UnsupportedVersion(v) => {
                write!(f, "Unsupported storage version: {}", v)
            }
            StorageError::EnvelopeTooShort => {
                write!(f, "Envelope too short: corrupt or incomplete storage file")
            }
            StorageError::DatabaseError(e) => write!(f, "Database error: {}", e),
            StorageError::EncryptionError(msg) => write!(f, "Encryption error: {}", msg),
            StorageError::DecryptionError(msg) => write!(f, "Decryption error: {}", msg),
            StorageError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            StorageError::Io(e) => write!(f, "I/O error: {}", e),
            StorageError::IdentityError(msg) => write!(f, "Identity error: {}", msg),
            StorageError::LockError(msg) => write!(f, "Lock error: {}", msg),
            StorageError::CorruptedEnvelope(msg) => write!(f, "Corrupted envelope: {}", msg),
            StorageError::Internal(msg) => write!(f, "Internal storage error: {}", msg),
        }
    }
}

impl std::error::Error for StorageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            StorageError::DatabaseError(e) => Some(e),
            StorageError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for StorageError {
    fn from(e: rusqlite::Error) -> Self {
        StorageError::DatabaseError(e)
    }
}

impl From<std::io::Error> for StorageError {
    fn from(e: std::io::Error) -> Self {
        StorageError::Io(e)
    }
}

/// Derives a 256-bit AES-GCM storage key from an attested SVID using BLAKE3 KDF (Feature 17).
pub fn derive_storage_key(svid: &Svid) -> [u8; 32] {
    let id_str = svid
        .spiffe_id()
        .map(|id| id.to_string())
        .unwrap_or_else(|_| svid.id.clone());
    blake3::derive_key(KDF_STORAGE_CONTEXT, id_str.as_bytes())
}

/// Derives a 256-bit AES-GCM storage key with caller-specified context.
pub fn derive_storage_key_with_context(svid: &Svid, context: &str) -> [u8; 32] {
    let id_str = svid
        .spiffe_id()
        .map(|id| id.to_string())
        .unwrap_or_else(|_| svid.id.clone());
    blake3::derive_key(context, id_str.as_bytes())
}

/// Pure-Rust authenticated cryptographic envelope for AES-256-GCM storage containers.
pub struct CryptoEnvelope;

impl CryptoEnvelope {
    /// Encrypts plaintext bytes into the envelope: `b"AURA" || version(1u8) || nonce(12B) || ciphertext || tag(16B)`.
    pub fn encrypt(
        key: &[u8; 32],
        nonce: &[u8; NONCE_LEN],
        plaintext: &[u8],
    ) -> Result<Vec<u8>, StorageError> {
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
        let gcm_nonce = Nonce::from_slice(nonce);

        let ciphertext_and_tag = cipher
            .encrypt(gcm_nonce, plaintext)
            .map_err(|e| StorageError::EncryptionError(format!("AES-256-GCM encryption failed: {}", e)))?;

        let mut envelope = Vec::with_capacity(HEADER_LEN + ciphertext_and_tag.len());
        envelope.extend_from_slice(ENCRYPTED_MAGIC);
        envelope.push(ENCRYPTED_VERSION);
        envelope.extend_from_slice(nonce);
        envelope.extend_from_slice(&ciphertext_and_tag);

        Ok(envelope)
    }

    /// Decrypts envelope bytes, validating magic bytes, version, and the authentication tag.
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
            StorageError::AuthenticationError(
                "AES-256-GCM authentication failed: wrong key, corrupted nonce, or tampered tag".to_string(),
            )
        })
    }
}

/// Thread-safe, identity-locked encrypted SQLite database container.
/// Operates entirely in RAM via an in-memory SQLite connection;
/// encrypts and decrypts on persistence boundaries so zero unencrypted bytes touch physical disk.
pub struct EncryptedDatabase {
    conn: Mutex<Connection>,
    path: Option<PathBuf>,
    key: [u8; 32],
}

impl fmt::Debug for EncryptedDatabase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EncryptedDatabase")
            .field("path", &self.path)
            .field("key", &"[REDACTED-STORAGE-KEY]")
            .finish()
    }
}

impl EncryptedDatabase {
    /// Opens an encrypted database at `path` using the given 256-bit key.
    /// If the file exists, decrypts the envelope and deserializes into an in-memory connection.
    /// If the file does not exist, initializes a new in-memory connection and saves the encrypted envelope to disk.
    pub fn open<P: AsRef<Path>>(path: P, key: [u8; 32]) -> Result<Self, StorageError> {
        let path_buf = path.as_ref().to_path_buf();

        if path_buf.exists() {
            let file_bytes = fs::read(&path_buf)?;
            let decrypted_bytes = CryptoEnvelope::decrypt(&key, &file_bytes)?;

            let mut conn = Connection::open_in_memory()?;
            let sz = decrypted_bytes.len();
            if sz > 0 {
                let ptr = unsafe { rusqlite::ffi::sqlite3_malloc64(sz as u64) as *mut u8 };
                if ptr.is_null() {
                    return Err(StorageError::Internal(
                        "sqlite3_malloc64 memory allocation failed".to_string(),
                    ));
                }
                unsafe {
                    std::ptr::copy_nonoverlapping(decrypted_bytes.as_ptr(), ptr, sz);
                    let non_null = std::ptr::NonNull::new(ptr).unwrap();
                    let owned = rusqlite::serialize::OwnedData::from_raw_nonnull(non_null, sz);
                    conn.deserialize(DatabaseName::Main, owned, false)?;
                }
            }

            Ok(Self {
                conn: Mutex::new(conn),
                path: Some(path_buf),
                key,
            })
        } else {
            let conn = Connection::open_in_memory()?;
            conn.execute_batch("PRAGMA user_version = 0;")?;
            let db = Self {
                conn: Mutex::new(conn),
                path: Some(path_buf),
                key,
            };
            db.flush()?;
            Ok(db)
        }
    }

    /// Opens an encrypted database locked directly to an attested SPIFFE SVID.
    pub fn open_with_svid<P: AsRef<Path>>(path: P, svid: &Svid) -> Result<Self, StorageError> {
        let key = derive_storage_key(svid);
        Self::open(path, key)
    }

    /// Creates an in-memory-only encrypted database with no backing disk path.
    pub fn open_in_memory(key: [u8; 32]) -> Result<Self, StorageError> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA user_version = 0;")?;
        Ok(Self {
            conn: Mutex::new(conn),
            path: None,
            key,
        })
    }

    /// Serializes the in-memory SQLite database, generates a fresh random 12-byte nonce,
    /// encrypts with AES-256-GCM, and writes the envelope atomically to disk using a tempfile and rename.
    pub fn flush(&self) -> Result<(), StorageError> {
        let mut nonce = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce);
        self.flush_with_nonce(&nonce)
    }

    /// Flushes the database using an explicit 12-byte nonce.
    pub fn flush_with_nonce(&self, nonce: &[u8; NONCE_LEN]) -> Result<(), StorageError> {
        let Some(ref path) = self.path else {
            return Ok(());
        };

        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::LockError(e.to_string()))?;
        let serialized_bytes = conn.serialize(DatabaseName::Main)?;
        let envelope = CryptoEnvelope::encrypt(&self.key, nonce, &serialized_bytes)?;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }

        // Atomic write via temporary sibling file followed by rename
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let temp_file = parent.join(format!(".tmp_{}", uuid::Uuid::new_v4()));
        fs::write(&temp_file, &envelope)?;
        fs::rename(&temp_file, path)?;

        Ok(())
    }

    /// Executes a SQL statement.
    pub fn execute<P: rusqlite::Params>(&self, sql: &str, params: P) -> Result<usize, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::LockError(e.to_string()))?;
        conn.execute(sql, params).map_err(StorageError::from)
    }

    /// Executes a batch of SQL statements.
    pub fn execute_batch(&self, sql: &str) -> Result<(), StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::LockError(e.to_string()))?;
        conn.execute_batch(sql).map_err(StorageError::from)
    }

    /// Queries multiple rows and maps them with a closure.
    pub fn query<P: rusqlite::Params, T, F>(
        &self,
        sql: &str,
        params: P,
        mut f: F,
    ) -> Result<Vec<T>, StorageError>
    where
        F: FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
    {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::LockError(e.to_string()))?;
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map(params, &mut f)?;
        let mut results = Vec::new();
        for r in rows {
            results.push(r?);
        }
        Ok(results)
    }

    /// Queries a single row from the database.
    pub fn query_row<P: rusqlite::Params, T, F>(
        &self,
        sql: &str,
        params: P,
        f: F,
    ) -> Result<T, StorageError>
    where
        F: FnOnce(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
    {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::LockError(e.to_string()))?;
        let result = conn.query_row(sql, params, f)?;
        Ok(result)
    }

    /// Provides exclusive access to the underlying `rusqlite::Connection`.
    pub fn with_conn<F, R>(&self, f: F) -> Result<R, StorageError>
    where
        F: FnOnce(&mut Connection) -> Result<R, StorageError>,
    {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::LockError(e.to_string()))?;
        f(&mut conn)
    }

    /// Returns the backing path, if any.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Returns the 256-bit encryption key.
    pub fn key(&self) -> &[u8; 32] {
        &self.key
    }
}
