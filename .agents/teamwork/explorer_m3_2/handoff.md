# Handoff Report: Milestone 3 Storage Encryption & Key Derivation (Features 15 & 17)

**Agent**: `explorer_m3_2` (Teamwork Explorer)  
**Parent Orchestrator ID**: `1732b97a-66c1-4a05-9f2b-afc669944583`  
**Working Directory**: `/home/jeff/aura/.agents/teamwork/explorer_m3_2`  
**Date**: 2026-10-11  

---

## 1. Observation

1. **User Request & Project Plan**:
   - `ORIGINAL_REQUEST.md` (§R3, lines 211-213): "SQLite database files and task ledgers at rest are encrypted with AES-256-GCM; raw disk byte inspection confirms ciphertext with no plaintext schema or task strings."
   - `PROJECT.md` (lines 28-30, 69-70):
     - Feature 15: `EncryptedDatabase` container over `rusqlite` using authenticated AES-256-GCM envelope (`[AURA][VERSION][NONCE][CIPHERTEXT+TAG]`).
     - Feature 17: `SPIFFE Key Derivation`: BLAKE3 KDF key derivation from attested `Svid` for identity-locked database encryption keys.
     - `EncryptedDatabase::open<P: AsRef<Path>>(path: P, key: [u8; 32]) -> Result<Self, StorageError>`.

2. **Existing Codebase State**:
   - `aura-core/src/identity/mod.rs` (lines 158-205, 248-251): Defines `Svid` struct with `id: String`, `trust_domain: String`, and `spiffe_id(&self) -> Result<SpiffeId, IdentityError>`. No key derivation method existed on `Svid`.
   - `aura-core/src/storage/` directory does not currently exist.
   - `aura-core/src/lib.rs` (lines 1-8): Currently exports `bus`, `config`, `hypervisor`, `identity`, `knowledge`, `runtime`, `supervisor`, `transport`. Missing `pub mod storage;`.
   - `aura-core/Cargo.toml` (lines 8-27): Has `rusqlite = { workspace = true }` and `blake3 = { workspace = true }`. Missing explicit `aes-gcm = "0.10.3"` and `getrandom = "0.2"`.
   - `Cargo.lock` (lines 30-38): Contains `aes = "0.8.4"`, `cipher = "0.4.4"`, `getrandom = "0.3.4"`/`"0.2"`, `ring = "0.17.14"`. Adding `aes-gcm = "0.10.3"` cleanly resolves to existing `aes 0.8.4` in the lockfile.

3. **Rusqlite In-Memory Lifecycle**:
   - `rusqlite-0.31.0/src/serialize.rs` (lines 68-133): Exposes `Connection::serialize(&self, schema: DatabaseName) -> Result<Data>` and `Connection::deserialize(&mut self, schema: DatabaseName, data: OwnedData, read_only: bool) -> Result<()>`.
   - These APIs enable 100% in-memory SQLite initialization and serialization without writing unencrypted temporary files to `/tmp`.

4. **Test Suite Alignment**:
   - `aura-core/tests/m5_e2e_opaque_test.rs` (lines 276-280, 2084-2128):
     - Constants: `ENCRYPTED_MAGIC = b"AURA"`, `ENCRYPTED_VERSION = 1u8`, `NONCE_LEN = 12`, `TAG_LEN = 16`.
     - Byte assertion indices:
       - Offset 0: corrupted magic byte (`envelope[0] = b'Z'`).
       - Offset 4: version byte.
       - Offset 6: corrupted nonce byte (`envelope[6] ^= 0xFF`).
       - Offset 17: corrupted ciphertext byte (`envelope[17] ^= 0xFF`).
       - Offset `len - 1`: corrupted auth tag byte (`envelope[last] ^= 0xFF`).
       - Minimum envelope length: 33 bytes (4 + 1 + 12 + 0 + 16).

---

## 2. Logic Chain

1. **Identity-Locking via BLAKE3 KDF (Feature 17)**:
   - Starting from Observation 1 and 2, every workload receives an attested `Svid` with a unique SPIFFE URI.
   - BLAKE3's KDF mode (`blake3::derive_key(context, material)`) provides 256-bit preimage resistance and collision resistance under domain separation context `"aura-storage-encryption-v1"`.
   - Deriving the 256-bit storage key from the canonicalized SPIFFE ID string guarantees that two distinct workloads will derive orthogonal 256-bit AES keys.
   - Therefore, opening a database encrypted by Workload A with Workload B's identity will fail AEAD authentication tag validation with mathematical certainty.

2. **Authenticated Envelope Persistence (Feature 15)**:
   - Based on Observations 1 and 4, the envelope `[AURA][VERSION: 1u8][NONCE: 12B][CIPHERTEXT][TAG: 16B]` stores the exact output of `aes_gcm::Aes256Gcm::encrypt`.
   - Header is 17 bytes (`[MAGIC: 4B][VERSION: 1B][NONCE: 12B]`).
   - Slicing `envelope[17..]` provides the exact ciphertext + tag slice for `aes_gcm::Aes256Gcm::decrypt`.
   - Any modification to ciphertext (index 17), nonce (index 6), or auth tag (index `len - 1`) causes AES-GCM GHASH authentication verification to fail, directly returning `StorageError::AuthenticationFailed`.
   - Any modification to magic bytes (index 0) or version (index 4) is caught by pre-flight envelope validation, returning `StorageError::InvalidMagic` or `StorageError::UnsupportedVersion`.

3. **Zero Plaintext Disk Leakage**:
   - Based on Observation 3, decrypting database bytes directly into memory and invoking `sqlite3_malloc64` + `deserialize` populates an in-memory `rusqlite::Connection`.
   - Writing to disk serializes via `conn.serialize()` in RAM, encrypts in RAM, and writes ciphertext to disk via temporary file rename (`.tmp_enc` -> `.db`).
   - Consequently, physical disk bytes never contain unencrypted SQLite headers (`SQLite format 3\0`), table names, or sensitive row strings.

---

## 3. Caveats

1. **Division of Labor**:
   - `explorer_m3_2` (this report) focused on Feature 15 (`EncryptedDatabase`) and Feature 17 (SPIFFE Key Derivation).
   - Peer agent `explorer_m3_3` is investigating Feature 16 (`TaskLedger`) and end-to-end integration test assertions. The blueprint in `strategy_report.md` §6 provides the exact contract for `TaskLedger` to wrap `Arc<EncryptedDatabase>`.
2. **Read-Only Investigation**:
   - Per Teamwork explorer constraints, no source code files or `Cargo.toml` were modified during this investigation. Implementation must be performed by an assigned worker agent (`worker_m3_1`).
3. **Database Concurrency Model**:
   - SQLite connections in `rusqlite` are single-connection in-memory databases wrapped in `std::sync::Mutex`. High-concurrency multithreaded writes are serialized by the mutex. This is completely standard and safe for single-workload microkernel sandboxes and task ledgers.

---

## 4. Conclusion

Features 15 and 17 have a clear, fully designed, production-ready implementation path:
1. Add `aes-gcm = "0.10.3"` and `getrandom = "0.2"` to `aura-core/Cargo.toml`.
2. Add `Svid::derive_storage_key(&self) -> Result<[u8; 32], IdentityError>` to `aura-core/src/identity/mod.rs`.
3. Create `aura-core/src/storage/` containing `mod.rs`, `encrypted.rs`, and `ledger.rs`.
4. Re-export `pub mod storage;` in `aura-core/src/lib.rs`.
5. Full implementation code, error types, byte-level envelope structures, and 11 unit tests have been documented in `/home/jeff/aura/.agents/teamwork/explorer_m3_2/strategy_report.md`.

---

## 5. Verification Method

Once implemented by `worker_m3_1`, independently verify the implementation with:

1. **Compilation Gate**:
   ```bash
   cargo check -p aura-core --jobs 1
   ```
   *Expected result*: Exits with code 0.

2. **Unit Test Gate**:
   ```bash
   cargo test -p aura-core --lib storage::encrypted --jobs 1
   ```
   *Expected result*: All 11 tests pass in <1s.

3. **Disk Ciphertext Inspection Gate**:
   Run `test_database_lifecycle_and_zero_plaintext_leakage`:
   Asserts:
   - Header is `b"AURA\x01"`.
   - String `SQLite format 3` is not found anywhere in raw file bytes.
   - Table names and secret payloads are not found anywhere in raw file bytes.
   - Unauthorized SVID cannot open or decrypt the file.

4. **Full Regression Gate**:
   ```bash
   cargo test -p aura-core --test m5_e2e_opaque_test --jobs 1
   cargo test -p aura-core --jobs 1
   ```
   *Expected result*: 100% tests passing with 0 regressions.
