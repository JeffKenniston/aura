# Milestone 3 Review & Adversarial Challenge Report: Features 13-17

**Reviewer**: `reviewer_m3_gen3_1` (teamwork_reviewer, teamwork_critic)  
**Parent Orchestrator**: `aede94a9-0e62-4bbd-a37a-72a2e3c4d3ae`  
**Working Directory**: `/home/jeff/aura/.agents/teamwork/reviewer_m3_gen3_1`  
**Date**: 2026-10-12  
**Target Subsystem**: Microkernel Zero-Trust Security & Storage Encryption (Features 13-17)  
**Verdict**: **REQUEST_CHANGES**

---

## Review Summary

**Verdict**: **REQUEST_CHANGES**

### Overview
Worker `worker_m3_1` implemented the foundational architectures for Milestone 3 (Features 13-17):
1. **Feature 13**: Vsock stream buffer bounds (`read_stream_with_limit_ext`, `read_stream_with_limit`, `VsockStreamConfig`) with instant truncation on buffer limit violations.
2. **Feature 14**: Pure-Rust TLS Egress Proxy (`EgressProxy`, `EgressProxyConfig`, `EgressProxyHandle`, `TLS_ALERT_ACCESS_DENIED`) re-exported in `aura-core/src/hypervisor/mod.rs` and wired into `FirecrackerVm` with RAII lifecycle cleanup.
3. **Features 15 & 17**: AES-256-GCM envelope encryption (`CryptoEnvelope`, `EncryptedDatabase`) and BLAKE3 KDF key derivation (`derive_storage_key`) from attested SPIFFE SVIDs.
4. **Feature 16**: Task lifecycle ledger (`TaskLedger`, `TaskEvent`, `TaskState`) persisted inside `EncryptedDatabase` with zero plaintext touching disk.
5. **Zero Mocks/Stubs/TODOs**: Strict verification confirms zero prohibited patterns (`todo`, `unimplemented`, `fixme`, `stub`) and zero mock logic in production paths.

**However**, execution of the mandatory workspace verification command:
```bash
cargo test -p aura-core --jobs 1
```
**FAILS** with exit code 101 due to compilation errors in `aura-core/tests/m3_empirical_challenge_test.rs`:
- `EncryptedDatabase` lacks an implementation of `std::fmt::Debug`, which breaks `Result::<EncryptedDatabase, StorageError>::unwrap_err()` (Rust E0277) across 7 assertion sites.
- `EncryptedDatabase::query_row` has an inconsistent signature taking `&[&dyn rusqlite::ToSql]` instead of generic `P: rusqlite::Params`, causing a type mismatch (Rust E0308) on standard empty parameter slices `[]`.

Per the repository constitution (AGENTS.md) and strict quality review gates, tests must compile and pass cleanly across all targets. Therefore, changes are requested to address these API defects.

---

## Findings

### [Critical] Finding 1: `EncryptedDatabase` missing `std::fmt::Debug` breaks caller error handling and workspace compilation
- **What**: `EncryptedDatabase` does not implement `std::fmt::Debug`. Consequently, any caller invoking `unwrap_err()` or `expect_err()` on a `Result<EncryptedDatabase, StorageError>` fails to compile with `error[E0277]: the trait Debug is not implemented for EncryptedDatabase`. This breaks `cargo test -p aura-core --jobs 1` during compilation of `aura-core/tests/m3_empirical_challenge_test.rs` with 7 compiler errors.
- **Where**: `aura-core/src/storage/encrypted.rs:178` (`pub struct EncryptedDatabase`) and `aura-core/src/storage/ledger.rs:105` (`pub struct TaskLedger`)
- **Why**: In Rust's standard library, `Result::<T, E>::unwrap_err` and `Result::<T, E>::expect_err` require `T: std::fmt::Debug`. Omitting `Debug` on core public types is an API anti-pattern that prevents test assertions and standard error inspection. Note that `conn: Mutex<Connection>` inside `EncryptedDatabase` does not allow automatic `#[derive(Debug)]` because `rusqlite::Connection` does not implement `Debug`, so `EncryptedDatabase` must implement `std::fmt::Debug` manually:
  ```rust
  impl std::fmt::Debug for EncryptedDatabase {
      fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
          f.debug_struct("EncryptedDatabase")
              .field("path", &self.path)
              .finish()
      }
  }
  ```
  And similarly for `TaskLedger`:
  ```rust
  impl std::fmt::Debug for TaskLedger {
      fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
          f.debug_struct("TaskLedger")
              .field("db", &self.db)
              .finish()
      }
  }
  ```
- **Suggestion**: Add manual implementations of `std::fmt::Debug` to `EncryptedDatabase` and `TaskLedger`.

### [Major] Finding 2: Inconsistent parameter type in `EncryptedDatabase::query_row`
- **What**: `EncryptedDatabase::execute` and `EncryptedDatabase::query` accept generic parameters `P: rusqlite::Params`, allowing `[]`, `rusqlite::params![]`, or slices. In contrast, `EncryptedDatabase::query_row` takes `params: &[&dyn rusqlite::ToSql]`, which rejects `[]` with `error[E0308]: mismatched types, expected &[&dyn ToSql], found [_; 0]`.
- **Where**: `aura-core/src/storage/encrypted.rs:326`
- **Why**: The parameter type inconsistency violates principle of least surprise and breaks standard parameter expressions.
- **Suggestion**: Update `query_row` to accept generic `P: rusqlite::Params`:
  ```rust
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
  ```

### [Major] Finding 3: Reconcile `aura-core/tests/m3_empirical_challenge_test.rs`
- **What**: `aura-core/tests/m3_empirical_challenge_test.rs` has compilation errors (8 errors across lines 404, 434, 446, 460, 477, 497, 517, 532).
- **Where**: `aura-core/tests/m3_empirical_challenge_test.rs:404` and assertion lines.
- **Why**: Once Findings 1 and 2 are resolved, ensure line 404 parameter passing (`[]`) compiles cleanly, run `cargo test -p aura-core --jobs 1`, and verify that all 15 challenge test cases pass.
- **Suggestion**: After fixing `EncryptedDatabase`, verify the challenge test compiles and passes.

### [Minor] Finding 4: CryptoEnvelope AAD Binding
- **What**: In `CryptoEnvelope::encrypt` and `decrypt`, the 5-byte header (`b"AURA\x01"`) is verified at the envelope framing level but is not bound as Associated Authenticated Data (AAD) in the AES-256-GCM cipher payload.
- **Where**: `aura-core/src/storage/encrypted.rs:134, 167`
- **Why**: While framing-level checks ensure magic and version conformance before decryption, binding `&envelope[..5]` into AEAD AAD cryptographically locks the container version to the authentication tag, adhering to defense-in-depth best practices.
- **Suggestion**: Bind envelope header as AAD in future revisions.

---

## Adversarial Challenge Report

**Overall Risk Assessment**: **MEDIUM** (core cryptographic primitives and sandboxing logic are solid, but public trait contract omissions break caller usability).

### Challenges

#### Challenge 1: `Result<EncryptedDatabase, StorageError>::unwrap_err()` in Caller Code
- **Assumption Challenged**: Callers can inspect error variants when opening encrypted databases.
- **Attack Scenario**: An application or test suite calls `EncryptedDatabase::open(untrusted_path, key).unwrap_err()` to handle or assert on `StorageError::InvalidMagic`.
- **Blast Radius**: Build failure at compile time (`E0277`). Caller cannot use standard Rust result unwrapping ergonomics.
- **Mitigation**: Implement `std::fmt::Debug` for `EncryptedDatabase`.

#### Challenge 2: Memory Flooding via Rogue Virtio-Vsock Streams (Feature 13)
- **Assumption Challenged**: Untrusted guest microVMs cannot exhaust host memory via vsock payload flooding.
- **Attack Scenario**: Malicious guest attempts to stream 100 MiB of random data over vsock port 5254.
- **Stress Test Result**: **PASS**. `read_stream_with_limit_ext` reads in bounded 64 KiB chunks and truncates instantly when `max_buffer_bytes` is reached, consuming zero additional heap.

#### Challenge 3: Egress Proxy SNI Bypasses & Prefix Spoofing (Feature 14)
- **Assumption Challenged**: Host proxy only permits exact allow-listed domains and official package mirrors.
- **Attack Scenario**: Attacker guest requests `evil-crates.io`, `crates.io.attacker.com`, or raw IP connections.
- **Stress Test Result**: **PASS**. `is_domain_allowed` strictly checks exact matches and `.`-prefixed subdomain boundaries (`target.as_bytes()[prefix_len - 1] == b'.'`). Unauthorized domains receive TLS Fatal Alert 49 (`0x15, 0x03, 0x03, 0x00, 0x02, 0x02, 0x31`) and immediate socket teardown.

#### Challenge 4: Forensic Disk Opacity & AES-256-GCM Nonce Reuse (Features 15-17)
- **Assumption Challenged**: SQLite page 1 and table contents never touch physical disk unencrypted, and consecutive flushes use fresh nonces.
- **Attack Scenario**: Inspect raw disk bytes of flushed database; verify whether `b"SQLite format 3\0"`, DDL strings, or task IDs appear; verify nonce collision across 100 flushes.
- **Stress Test Result**: **PASS**. Raw disk inspection reveals 100% ciphertext with Shannon entropy > 7.95 bits/byte. Plaintext needles are 0% detected. Each flush generates a fresh 12-byte nonce via `OsRng`.

---

## 1. Observation

Direct empirical observations made across the Milestone 3 implementation files and tool outputs:

### A. Code Citations
1. **`aura-core/src/hypervisor/firecracker.rs` (Feature 13)**:
   - Lines 19-37: `VsockStreamConfig` with `DEFAULT_MAX_VSOCK_STREAM_BYTES = 16 MiB`.
   - Lines 423-477: `read_stream_with_limit_ext` and `read_stream_with_limit` reading in 64 KiB chunks bounded by `max_bytes` with instant truncation.
   - Lines 586-663: `FirecrackerVm` fields `egress_proxy: Option<EgressProxyHandle>` and `vsock_stream_config: VsockStreamConfig`. `start_egress_proxy`, `stop_egress_proxy`.
   - Lines 819-860 & 898-902: `teardown()` calls `stop_egress_proxy()`, and `Drop for FirecrackerVm` calls `teardown()`.

2. **`aura-core/src/hypervisor/proxy.rs` & `mod.rs` (Feature 14)**:
   - `aura-core/src/hypervisor/mod.rs:15-18`: Re-exports `EgressProxy`, `EgressProxyConfig`, `EgressProxyHandle`, `DEFAULT_ALLOWED_DOMAINS`, `TLS_ALERT_ACCESS_DENIED`.
   - `proxy.rs:33-39`: Defines `TLS_ALERT_ACCESS_DENIED` (`[0x15, 0x03, 0x03, 0x00, 0x02, 0x02, 0x31]`).
   - `proxy.rs:107-132`: `is_domain_allowed` enforcing strict exact and subdomain matching.
   - `proxy.rs:136-234`: Pure-Rust RFC 5246/6066/8446 SNI parsing (`extract_sni`).

3. **`aura-core/src/storage/` (Features 15-17)**:
   - `encrypted.rs:104-110`: `derive_storage_key` using BLAKE3 KDF `blake3::derive_key("aura-storage-encryption-v1", svid.spiffe_id().as_bytes())`.
   - `encrypted.rs:122-173`: `CryptoEnvelope` defining `b"AURA"` + `1u8` + 12-byte nonce + AES-256-GCM ciphertext + 16-byte tag.
   - `encrypted.rs:178-360`: `EncryptedDatabase` operating in-memory with `rusqlite::Connection::open_in_memory()`, deserializing via `sqlite3_malloc64` and `conn.deserialize()`, serializing via `conn.serialize()`, generating fresh random nonces via `OsRng`, and atomic write via temporary file and rename.
   - `ledger.rs:105-200`: `TaskLedger` backed by `EncryptedDatabase` with auto-increment ID and indexed by `task_id`. Immediate flush on `append_event`.

### B. Constitutional Compliance Scan (AGENTS.md)
1. Keyword search across `aura-core/src/hypervisor/` and `aura-core/src/storage/`:
   - `todo`: 0 matches.
   - `fixme`: 0 matches.
   - `unimplemented`: 0 matches.
   - `stub`: 0 matches.
2. `mock` keyword search:
   - 0 matches in production code paths.
   - Only found in test fixture naming (`mock_upstream`, `mock_kvm_readonly`) within `#[cfg(test)]`.

### C. Independent Empirical Test Results
1. `cargo test -p aura-core --lib --jobs 1`:
   - **Result**: `154 passed; 0 failed; 0 ignored; finished in 1.88s`.
2. `cargo test -p aura-core --test m5_e2e_opaque_test`:
   - **Result**: `170 passed; 0 failed; 0 ignored; finished in 0.15s`.
3. `cargo test -p aura-core --test m5_empirical_milestone3_stress_test`:
   - **Result**: `10 passed; 0 failed; 0 ignored; finished in 1.15s`.
4. `cargo test --manifest-path deploy/images/vsock-screenshot-daemon/Cargo.toml`:
   - **Result**: `39 passed; 0 failed; 0 ignored; finished in 4.07s`.
5. `cargo test --manifest-path tools/ephemeral-tunnel/Cargo.toml`:
   - **Result**: `14 passed; 0 failed; 0 ignored; finished in 0.58s`.
6. `/home/jeff/aura/aura-sdk/.venv/bin/pytest aura-sdk/tests`:
   - **Result**: `107 passed, 1 warning in 11.72s`.
7. `/home/jeff/aura/aura-sdk/.venv/bin/ruff check aura-sdk/aura_sdk aura-sdk/tests`:
   - **Result**: `All checks passed!`.
8. `cargo test -p aura-core --jobs 1`:
   - **Result**: **FAILED** (Exit code 101, 8 compilation errors on `tests/m3_empirical_challenge_test.rs`).

---

## 2. Logic Chain

1. **Observation**: Running `cargo test -p aura-core --jobs 1` fails compilation with:
   - 7 instances of `error[E0277]: the trait Debug is not implemented for EncryptedDatabase`
   - 1 instance of `error[E0308]: mismatched types: expected &[&dyn ToSql], found [_; 0]`
2. **Inference**: Cargo compiles all integration test files in `tests/*.rs` during `cargo test -p aura-core`. Because `aura-core/tests/m3_empirical_challenge_test.rs` resides in `tests/`, its failure to compile prevents the mandatory milestone verification gate `cargo test -p aura-core --jobs 1` from completing.
3. **Inference**: The underlying cause is that `EncryptedDatabase` does not implement `std::fmt::Debug`, which is a required trait bound for `Result::unwrap_err()`. Furthermore, `query_row`'s rigid parameter signature creates unnecessary friction compared to `execute` and `query`.
4. **Conclusion**: While all unit tests, SDK tests, and daemon tests pass when isolated, the primary crate test command requested by the project gates fails. Changes must be requested to implement `std::fmt::Debug`, reconcile `query_row`, and achieve a 100% clean build of `cargo test -p aura-core --jobs 1`.

---

## 3. Caveats

- `m3_empirical_challenge_test.rs` was introduced as an untracked challenge test during milestone adversarial evaluation. However, because it tests valid public API usage of `EncryptedDatabase` (`unwrap_err`, `query_row`), its compilation failure reflects genuine API omissions in `EncryptedDatabase`.
- When individual targets are specified (`--lib`, `--test m5_e2e_opaque_test`, `--test m5_empirical_milestone3_stress_test`), 100% of tests pass (334 passing Rust tests across core, 39 daemon, 14 tunnel, 107 Python).

---

## 4. Conclusion

Milestone 3 has achieved strong architectural implementation for Features 13-17 with zero mocks and verified zero-trust security boundaries. However, due to the failure of `cargo test -p aura-core --jobs 1` caused by missing `std::fmt::Debug` on `EncryptedDatabase` and `query_row` parameter type rigidity:

**Verdict**: **REQUEST_CHANGES**

### Required Action Items for Worker:
1. Implement `std::fmt::Debug` on `EncryptedDatabase` in `aura-core/src/storage/encrypted.rs` and `TaskLedger` in `aura-core/src/storage/ledger.rs`.
2. Update `EncryptedDatabase::query_row` to accept generic `P: rusqlite::Params`.
3. Verify that `cargo test -p aura-core --jobs 1` compiles and passes with zero errors across all targets (including `m3_empirical_challenge_test.rs`).

---

## 5. Verification Method

To independently reproduce this verification:

1. **Reproduce compilation failure**:
   ```bash
   cargo test -p aura-core --jobs 1
   ```
   *Expected currently*: Exit code 101 with 8 compilation errors on `m3_empirical_challenge_test.rs`.

2. **Verify isolated lib and stress test pass**:
   ```bash
   cargo test -p aura-core --lib --jobs 1
   cargo test -p aura-core --test m5_e2e_opaque_test
   cargo test -p aura-core --test m5_empirical_milestone3_stress_test
   ```
   *Expected*: 154 passed, 170 passed, 10 passed.

3. **Verify Python SDK test suite**:
   ```bash
   /home/jeff/aura/aura-sdk/.venv/bin/pytest aura-sdk/tests
   /home/jeff/aura/aura-sdk/.venv/bin/ruff check aura-sdk/aura_sdk aura-sdk/tests
   ```
   *Expected*: 107 passed; "All checks passed!".

4. **Invalidation condition**:
   Adding `impl std::fmt::Debug for EncryptedDatabase` and updating `query_row` to generic `P: rusqlite::Params` will allow `cargo test -p aura-core --jobs 1` to compile and pass cleanly.
