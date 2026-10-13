# Handoff Report: Survey Requirements R3 & R4 (Phase M5 Hardening)

**Agent**: `teamwork_preview_explorer_survey_3`  
**Recipient**: `dae2eddf-f903-4d1f-b5f4-42a3e7019103` (orchestrator)  
**Date**: 2026-10-11  
**Handoff Type**: Hard (Investigation complete)

---

## 1. Observation

1. **Vsock Stream Buffer Handling**:
   - `aura-core/src/hypervisor/firecracker.rs:382-386`:
     ```rust
     let mut buffer = Vec::new();
     if let Err(e) = stream.read_to_end(&mut buffer).await {
         eprintln!("Failed reading vsock screenshot stream: {}", e);
         return;
     }
     ```
     `read_to_end` reads unbounded byte streams into memory. There is no configurable memory ceiling or instant truncation mechanism.

2. **MicroVM Network Egress TLS Proxy**:
   - `aura-core/src/hypervisor/proxy.rs:8-420`: Pure-Rust `EgressProxy` is already substantially implemented.
   - `DEFAULT_ALLOWED_DOMAINS` allowlists packages (`crates.io`, `pypi.org`, `deb.debian.org`, etc.).
   - `TLS_ALERT_ACCESS_DENIED` (`[0x15, 0x03, 0x03, 0x00, 0x02, 0x02, 0x31]`) drops unauthorized SNI handshakes with TLS Fatal Alert 49.
   - `EgressProxy::extract_sni` parses RFC 5246/6066/8446 ClientHello frames.
   - `hypervisor/proxy.rs` has 7 passing unit tests. However, `hypervisor/mod.rs` does not re-export proxy symbols.

3. **Storage Encryption & Task Ledgers**:
   - `aura-core/src/knowledge/cache.rs:101`: `SemanticCache::open_file` calls unencrypted `rusqlite::Connection::open(path)`.
   - `aura-core/src/identity/mod.rs:524-563`: `fetch_svid()` retrieves `Svid` with `id`, `cert_chain_der`, and `private_key_der`.
   - No module currently exists for encrypted SQLite persistence, AES-256-GCM storage containers, or `TaskLedger`.

4. **SPIRE Federation & OIDC Tokens**:
   - `aura-core/src/identity/mod.rs:670-694`: `FederationManager` contains `federated_domains: DashMap<String, Vec<u8>>` for trust domain bundles.
   - No structures exist for enterprise OIDC token validation or SPIRE federation claim-to-scope mappings.

5. **Transport 1-RTT UDP Failover**:
   - `aura-core/src/transport/http3.rs:93-106`: TCP listener currently returns static `HTTP/1.1 200 OK\r\nAlt-Svc: ...` and closes without processing MCP requests.
   - `aura-core/src/transport/mcp.rs:149-270`: `McpServer` implements `handle_http_request` but is not connected to the TCP fallback loop.
   - No resilient client exists to detect UDP/QUIC failure within 1 RTT and transparently retry in-flight tool calls on TCP.

6. **Workspace Test Baseline**:
   - `cargo test -p aura-core --jobs 1`: 146 passed, 0 failed (126 lib tests + 20 integration tests across 4 test binaries).
   - `cargo test --manifest-path tools/ephemeral-tunnel/Cargo.toml`: 6 passed, 0 failed.
   - `cargo test --manifest-path deploy/images/vsock-screenshot-daemon/Cargo.toml`: 1 passed, 0 failed.
   - `/home/jeff/aura/aura-sdk/.venv/bin/pytest aura-sdk/tests`: 84 passed, 0 failed.
   - `/home/jeff/aura/aura-sdk/.venv/bin/ruff check aura-sdk/aura_sdk aura-sdk/tests`: clean.

---

## 2. Logic Chain

1. **Vsock Memory Protection**: Because `stream.read_to_end` allows unbounded reads, a malicious guest can exhaust host RAM. Enforcing `VsockStreamConfig { max_buffer_bytes }` and truncating incoming streams when the limit is breached guarantees constant memory bounds without panics.
2. **Egress Zero-Trust Boundary**: Because `EgressProxy` already validates SNI against `DEFAULT_ALLOWED_DOMAINS` and emits `TLS_ALERT_ACCESS_DENIED`, re-exporting it and providing a runtime attachment method satisfies R3 egress enforcement.
3. **Encrypted Persistence**: Because raw SQLite files contain unencrypted schemas and strings, introducing `EncryptedDatabase` with AES-256-GCM authenticated encryption ensures raw disk bytes contain only ciphertext `[AURA][VERSION][NONCE][CIPHERTEXT+TAG]`. Deriving the encryption key via BLAKE3 KDF from the attested `Svid` ensures identity-locked persistence. Building `TaskLedger` on top of `EncryptedDatabase` secures runtime task ledgers at rest.
4. **Federation & OIDC Integration**: Extending `FederationManager` with `OidcProviderConfig`, `SpireFederationMapping`, and `verify_oidc_token` enables external enterprise identity tokens to authenticate into scoped, un-escalatable SVIDs.
5. **Transport Resilience**: Connecting `McpServer` to the TCP listener in `http3.rs` enables full tool execution over TCP fallback. Adding `ResilientMcpTransportClient` with a 1-RTT UDP failure timeout (<100ms) and automatic in-flight request replay on TCP ensures zero dropped tool calls when UDP is blocked.

---

## 3. Caveats

1. **Hardware KVM / Firecracker Execution in CI/Container**: KVM virtual devices require hardware virtualization support (`/dev/kvm`). In non-KVM environments, `AURA_SKIP_KVM_VALIDATION=1` allows host-subshell and mock vsock execution as supported by existing tests.
2. **OIDC Token Signing Algorithms**: The proposed OIDC token verification handles standard HMAC-SHA256 (HS256) and RSA/Ed25519 token signatures without requiring external heavyweight network dependencies.
3. **SQLite Serialized Backups**: In-memory SQLite databases will use atomic serialization / backup flushes to disk through the AES-256-GCM envelope, ensuring ACID durability and instant encryption.

---

## 4. Conclusion

The architectural boundaries, existing components, and missing gaps for M5 requirements R3 and R4 have been fully surveyed and cataloged in `/home/jeff/aura/.agents/teamwork/teamwork_preview_explorer_survey_3/survey_report.md`.

Concrete implementation work for M5 is partitioned into:
1. `hypervisor/firecracker.rs`: Add `VsockStreamConfig` and `read_stream_with_limit`.
2. `hypervisor/mod.rs`: Re-export `EgressProxy` types.
3. `storage/` (`encrypted.rs`, `ledger.rs`): Implement AES-256-GCM encrypted database container and `TaskLedger`.
4. `identity/mod.rs`: Add `SpireFederationMapping`, `OidcProviderConfig`, and `verify_oidc_token`.
5. `transport/` (`http3.rs`, `client.rs`): Wire `McpServer` into TCP listener; add `ResilientMcpTransportClient` with 1-RTT failover and in-flight replay.

---

## 5. Verification Method

To verify the survey findings independently:
1. Inspect survey report: `view_file /home/jeff/aura/.agents/teamwork/teamwork_preview_explorer_survey_3/survey_report.md`
2. Run baseline tests: `cargo test -p aura-core --jobs 1`
3. Verify existing egress proxy tests: `cargo test -p aura-core --lib hypervisor::proxy::tests`
4. Verify existing vsock screenshot tests: `cargo test -p aura-core --lib hypervisor::firecracker::tests::test_screenshot`
