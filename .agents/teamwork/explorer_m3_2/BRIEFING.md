# BRIEFING — 2026-10-11T23:36:00Z

## Mission
Investigate Milestone 3: Storage Encryption & Key Derivation (Features 15 & 17) in Aura, designing SPIFFE-derived key derivation (BLAKE3 KDF) and AES-256-GCM envelope-encrypted SQLite database storage.

## 🔒 My Identity
- Archetype: explorer
- Roles: investigation, synthesis
- Working directory: /home/jeff/aura/.agents/teamwork/explorer_m3_2
- Original parent: 1732b97a-66c1-4a05-9f2b-afc669944583
- Milestone: Milestone 3 (Storage Encryption & Key Derivation)

## 🔒 Key Constraints
- Read-only investigation — do NOT implement directly in project source code
- Strictly adhere to AGENTS.md: Production-ready code design, ABSOLUTELY NO mocks, stubs, or TODOs
- Follow Aura domain rules (iam domain, security invariants)

## Current Parent
- Conversation ID: 1732b97a-66c1-4a05-9f2b-afc669944583
- Updated: not yet

## Investigation State
- **Explored paths**:
  - `aura-core/Cargo.toml`, `Cargo.lock` (crypto and SQLite dependencies)
  - `aura-core/src/identity/mod.rs` (`Svid`, `SpiffeId`, dynamic attestation)
  - `rusqlite-0.31.0` source (`serialize.rs`, `backup.rs`, in-memory deserialization)
  - `aura-core/tests/m5_e2e_opaque_test.rs` (Feature 12/13/15/17 test assertions and offsets)
  - `aura-core/src/lib.rs` (module structure)
- **Key findings**:
  - Feature 17: BLAKE3 KDF `blake3::derive_key("aura-storage-encryption-v1", svid.canonical_id()?.as_bytes())` guarantees 256-bit orthogonal subkeys, identity-locking storage to the presenting workload.
  - Feature 15: Envelope format `[b"AURA"][1u8][nonce: 12B][ciphertext + 16B auth tag]` matches AES-256-GCM encrypt/decrypt slice alignment byte-for-byte.
  - Zero-Disk-Leakage Invariant: Decrypting in RAM and deserializing into `rusqlite::Connection::open_in_memory()` via `sqlite3_malloc64` + `deserialize` guarantees unencrypted plaintext database files are NEVER created on physical storage.
  - Serializing via `conn.serialize()` and atomic temp file rename (`.tmp_enc` -> `.db`) ensures crash-safe persistence.
- **Unexplored areas**: None. Full concrete implementation blueprints and test specifications produced.

## Key Decisions Made
- Use `aes-gcm = "0.10.3"` and `getrandom = "0.2"` in `aura-core/Cargo.toml`.
- Structure new module as `aura-core/src/storage/` (`mod.rs`, `encrypted.rs`, `ledger.rs`).
- Export `pub mod storage;` in `aura-core/src/lib.rs`.
- Defined full error enum `StorageError` implementing `Display` and `std::error::Error` manually.

## Artifact Index
- `DISPATCH.md` — Record of dispatch instructions
- `progress.md` — Liveness heartbeat and progress tracking
- `strategy_report.md` — Detailed implementation blueprint with complete production-ready code snippets and test specifications
- `handoff.md` — Standard 5-component handoff report
