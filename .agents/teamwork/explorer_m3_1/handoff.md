# Handoff Report: Milestone 3 (Features 13 & 14) Investigation

**Agent**: `explorer_m3_1`  
**Role**: Teamwork Explorer (Read-only Investigation)  
**Parent Orchestrator**: `1732b97a-66c1-4a05-9f2b-afc669944583`  
**Date**: 2026-10-11  
**Artifact**: `/home/jeff/aura/.agents/teamwork/explorer_m3_1/strategy_report.md`  

---

## 1. Observation

1. **Unbounded Vsock Stream Buffering in `firecracker.rs`**:
   - In `aura-core/src/hypervisor/firecracker.rs:382-386`:
     ```rust
     let mut buffer = Vec::new();
     if let Err(e) = stream.read_to_end(&mut buffer).await {
         eprintln!("Failed reading vsock screenshot stream: {}", e);
         return;
     }
     ```
     `stream.read_to_end(&mut buffer)` has no memory limit. Any rogue guest daemon can stream unbounded data, exhausting host memory.
   - `VsockStreamConfig`, `VsockStreamError`, and `read_stream_with_limit` do not currently exist in `aura-core`.

2. **Existing Pure-Rust Egress Proxy in `proxy.rs`**:
   - In `aura-core/src/hypervisor/proxy.rs`:
     - `DEFAULT_ALLOWED_DOMAINS` (lines 9-30) covers official registries (`crates.io`, `pypi.org`, `deb.debian.org`, `archive.ubuntu.com`, `dl-cdn.alpinelinux.org`, `github.com`).
     - `TLS_ALERT_ACCESS_DENIED` (lines 33-39) defines TLS Fatal Alert 49 (Access Denied):
       `[0x15, 0x03, 0x03, 0x00, 0x02, 0x02, 0x31]`.
     - `EgressProxy::extract_sni` (lines 136-234) parses TLS ClientHello RFC 5246/6066/8446 in pure Rust with strict bounds checks.
     - `EgressProxy::is_domain_allowed` (lines 107-132) enforces exact matches, wildcard subdomains, and rejects prefix collisions (e.g. `evil-crates.io` is blocked).
     - `EgressProxy::handle_connection` (lines 311-419) sends `TLS_ALERT_ACCESS_DENIED` on unauthorized domains and tunnels allowed traffic.
     - Unit tests in `proxy.rs:423-645` all pass cleanly.

3. **Missing Re-Exports in `mod.rs`**:
   - In `aura-core/src/hypervisor/mod.rs:1-8`:
     ```rust
     pub mod firecracker;
     pub mod proxy;
     pub mod wasm;

     pub use firecracker::{
         format_redacted_handle, should_skip_kvm_validation, validate_kvm_at_path, validate_kvm_support,
         BashCommandOutput, BashSessionHandle, BashSessionManager, KvmError,
     };
     ```
     `EgressProxy`, `EgressProxyConfig`, `EgressProxyHandle`, and constants are NOT re-exported in `aura-core::hypervisor`.
   - `FirecrackerVm` currently lacks `egress_proxy: Option<EgressProxyHandle>` and lifecycle methods (`start_egress_proxy`, `stop_egress_proxy`).

4. **Test Suite Baseline**:
   - `cargo test -p aura-core --lib hypervisor`: 36 passed, 0 failed.
   - `cargo test -p aura-core --lib`: 135 passed, 0 failed.
   - `cargo test -p aura-core --test m5_e2e_opaque_test`: 170 passed, 0 failed.
   - In `aura-core/tests/m5_e2e_opaque_test.rs:239-273`, an opaque standalone `read_stream_with_limit` exists for testing F10.

---

## 2. Logic Chain

1. **Memory Protection Chain**:
   - From Observation 1, `read_to_end` exposes host hypervisor memory to arbitrary allocation from guest vsock streams.
   - Introducing `VsockStreamConfig { max_buffer_bytes: usize }` and `read_stream_with_limit_ext` that reads in small chunks (e.g. 4096 bytes) and terminates reading immediately once `max_buffer_bytes` is reached guarantees that heap allocations never exceed `max_buffer_bytes`.
   - Instantly truncating the incoming stream and returning the truncated payload enables downstream consumers (like `extract_png_bytes`) to safely evaluate or reject the data without panicking.
   - Refactoring `start_screenshot_listener` to delegate to `start_screenshot_listener_with_config` bounds screenshot listener allocations while preserving full backward compatibility.

2. **Egress Proxy Re-Export & Lifecycle Wiring Chain**:
   - From Observation 2, `proxy.rs` is functionally complete and correct with passing tests, but from Observation 3, callers must navigate into `aura_core::hypervisor::proxy` rather than accessing `EgressProxy` via `aura_core::hypervisor`.
   - Re-exporting `EgressProxy`, `EgressProxyConfig`, `EgressProxyHandle`, `DEFAULT_ALLOWED_DOMAINS`, and `TLS_ALERT_ACCESS_DENIED` in `aura-core/src/hypervisor/mod.rs` makes the egress proxy a top-level hypervisor abstraction.
   - Following the pattern established by `CdpVsockForwarder` in `FirecrackerVm`, adding `egress_proxy: Option<EgressProxyHandle>` and `start_egress_proxy` / `stop_egress_proxy` binds the proxy's lifecycle to the microVM. Calling `stop_egress_proxy()` in `teardown()` ensures RAII cleanup upon microVM termination or drop.

---

## 3. Caveats

1. **Features 15-17 Handled Separately**: Storage encryption (Features 15 & 17) and Task Ledger (Feature 16) are under investigation by peer explorers `explorer_m3_2` and `explorer_m3_3`. Features 13 & 14 have zero compile-time dependencies on the storage module.
2. **KVM Validation in CI**: KVM tests in `firecracker.rs` rely on `should_skip_kvm_validation()` (`AURA_SKIP_KVM_VALIDATION=1`). The vsock and proxy features operate purely in userspace (Unix domain sockets and TCP loopback) and do not require `/dev/kvm`.
3. **Read Signature Flexibility**: To support both callers expecting `Result<Vec<u8>, VsockStreamError>` (per `PROJECT.md` interface contract) and callers expecting the truncation flag `Result<(Vec<u8>, bool), VsockStreamError>`, both `read_stream_with_limit` and `read_stream_with_limit_ext` are specified.

---

## 4. Conclusion

Features 13 and 14 are well-scoped, architecturally sound, and ready for immediate implementation by `worker_m3_1`.
A complete implementation blueprint with exact signatures, algorithms, lifecycle hooks, and unit tests is documented in:
`/home/jeff/aura/.agents/teamwork/explorer_m3_1/strategy_report.md`.

---

## 5. Verification Method

To verify the strategy once implemented:

```bash
# 1. Hypervisor module unit tests (must include new vsock truncation and egress proxy lifecycle tests)
cargo test -p aura-core --lib hypervisor

# 2. Entire aura-core unit tests (zero regressions across all 135+ tests)
cargo test -p aura-core --lib

# 3. Comprehensive opaque E2E test suite (170 tests passing)
cargo test -p aura-core --test m5_e2e_opaque_test

# 4. Check syntax and warnings
cargo check -p aura-core
```
