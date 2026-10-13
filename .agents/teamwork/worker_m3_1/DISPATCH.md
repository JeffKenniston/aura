## 2026-10-11T23:36:42Z
You are worker_m3_1, a teamwork_preview_worker implementing Milestone 3: Microkernel Zero-Trust Security & Storage Encryption (Features 13-17) in Aura.

Your working directory is: /home/jeff/aura/.agents/teamwork/worker_m3_1
Parent Orchestrator Conversation ID: 1732b97a-66c1-4a05-9f2b-afc669944583

MANDATORY INTEGRITY WARNING:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

MANDATORY: Read the authoritative user request first:
/home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md

Also read:
- /home/jeff/aura/PROJECT.md
- /home/jeff/aura/.agents/teamwork/explorer_m3_1/strategy_report.md
- /home/jeff/aura/.agents/teamwork/explorer_m3_2/strategy_report.md
- /home/jeff/aura/.agents/teamwork/explorer_m3_3/strategy_report.md
- /home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-host-domain/SKILL.md
- /home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-iam-domain/SKILL.md

File Write Ownership (Exclusive):
- aura-core/Cargo.toml
- aura-core/src/lib.rs
- aura-core/src/hypervisor/firecracker.rs
- aura-core/src/hypervisor/mod.rs
- aura-core/src/hypervisor/proxy.rs
- aura-core/src/storage/mod.rs
- aura-core/src/storage/encrypted.rs
- aura-core/src/storage/ledger.rs

Tasks:
1. In `aura-core/Cargo.toml`:
   - Add `aes-gcm = "0.10.3"`.
   - Update `rusqlite` dependency to ensure `features = ["backup", "serialize"]`.

2. In `aura-core/src/hypervisor/firecracker.rs`:
   - Implement `VsockStreamConfig { pub max_buffer_bytes: usize }` with default (e.g. 16MB).
   - Implement `VsockStreamError` (enum with `LimitExceeded { limit: usize, received: usize }`, `Io(std::io::Error)`, etc.).
   - Implement `read_stream_with_limit<R: AsyncRead + Unpin>(&mut R, max_bytes: usize) -> Result<Vec<u8>, VsockStreamError>`:
     Reads stream in chunks (e.g. 64KB). If total bytes exceed `max_bytes`, instantly truncates stream and returns error or truncated slice without unbounded memory consumption or panic.
   - Refactor `start_screenshot_listener` to use `VsockStreamConfig`.
   - Add unit tests for `read_stream_with_limit` (happy path, exact limit, limit exceeded truncation).

3. In `aura-core/src/hypervisor/proxy.rs` & `aura-core/src/hypervisor/mod.rs`:
   - Re-export `EgressProxy`, `EgressProxyConfig`, `EgressProxyHandle`, `TLS_ALERT_ACCESS_DENIED` in `aura_core::hypervisor`.
   - Wire `egress_proxy: Option<EgressProxyHandle>` into `FirecrackerVm` lifecycle with RAII cleanup on stop/drop.

4. In `aura-core/src/lib.rs` & `aura-core/src/storage/`:
   - Add `pub mod storage;` in `aura-core/src/lib.rs`.
   - Implement `aura-core/src/storage/encrypted.rs`:
     * Implement `derive_storage_key(svid: &Svid) -> [u8; 32]`:
       Derives 256-bit key using BLAKE3 KDF: `blake3::derive_key("aura-storage-encryption-v1", svid.spiffe_id().as_bytes())`.
     * Implement `EncryptedDatabase`:
       - Envelope format: `b"AURA"` (4 bytes) + `1u8` (version 1) + 12-byte random nonce + ciphertext (AES-256-GCM encrypted serialized sqlite db) + 16-byte auth tag.
       - Operates in RAM via `rusqlite::Connection::open_in_memory()`.
       - On `open(path, key)`: if file exists, reads envelope, verifies magic/version, decrypts with AES-256-GCM, deserializes into in-memory sqlite connection via `deserialize`. Returns `StorageError::AuthenticationError` on wrong key, corrupted nonce, or tampered tag. If file does not exist, initializes new in-memory connection and saves encrypted file on disk.
       - On `flush(&self) -> Result<(), StorageError>`: serializes in-memory connection via `conn.serialize()`, generates fresh random 12-byte nonce, encrypts with AES-256-GCM, writes envelope atomically to disk using a tempfile and rename.
       - Implement `StorageError` with proper error types.
   - Implement `aura-core/src/storage/ledger.rs`:
     * Implement `TaskState` (Pending, Running, Completed, Failed, Cancelled) and `TaskEvent` (`task_id`, `state`, `timestamp`, `metadata`, `transition_reason`).
     * Implement `TaskLedger`:
       - Backed by `Arc<EncryptedDatabase>`.
       - `new(db: Arc<EncryptedDatabase>) -> Result<Self, StorageError>` creates schema if not exists: `CREATE TABLE IF NOT EXISTS task_events (id INTEGER PRIMARY KEY AUTOINCREMENT, task_id TEXT NOT NULL, state TEXT NOT NULL, timestamp INTEGER NOT NULL, metadata TEXT, transition_reason TEXT);`
       - `append_event(&self, event: &TaskEvent) -> Result<(), StorageError>`: inserts record and calls `db.flush()`.
       - `get_events(&self, task_id: &str) -> Result<Vec<TaskEvent>, StorageError>`: queries and returns events for given `task_id` ordered by id.
   - Add tests in `storage/` covering:
     * Key derivation from SVID
     * Roundtrip storage encryption and decryption
     * Wrong key rejection and tampering rejection
     * Raw disk byte inspection: read disk file bytes, assert `b"AURA\x01"` header present, assert `b"SQLite format 3\0"` NOT present anywhere in file bytes, assert table names (`task_events`) and task strings NOT present in raw bytes.

5. Verification Commands:
   - `cargo test -p aura-core --jobs 1`
   - `cargo test -p aura-core --test m5_e2e_opaque_test`
   - `cargo test --manifest-path deploy/images/vsock-screenshot-daemon/Cargo.toml`
   - `cargo test --manifest-path tools/ephemeral-tunnel/Cargo.toml`
   - `/home/jeff/aura/aura-sdk/.venv/bin/pytest aura-sdk/tests`
   - `/home/jeff/aura/aura-sdk/.venv/bin/ruff check aura-sdk/aura_sdk aura-sdk/tests`

6. Output:
   - Update `progress.md` in your working directory.
   - Write a complete `handoff.md` in your working directory detailing changes made, test commands and passing outputs, and layout compliance.
   - Send completion message to parent with report path.
