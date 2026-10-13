# Project: Aura Phase M5 (Hardening)

## Architecture
The Aura Phase M5 (Hardening) release establishes microkernel zero-trust isolation, ephemeral credential containment, sandboxed visual automation, and resilient identity/transport federation across five core subsystems:
1. **Ephemeral Credential Lifecycle & Out-of-Band Secret Isolation**: Task-scoped credential minting via `EphemeralTunnelService` with cryptographic SPIFFE SVID non-escalation, opaque handle redaction (`[REDACTED-TOKEN-...]`), RAII drop-guarded revocation, and out-of-band environment injection into sandbox runtimes for secure VCS PR creation (`GitManager::open_pr`).
2. **Sandboxed GUI Automation & Visual Redaction**: Firecracker microVM guest vsock daemon (`vsock-screenshot-daemon`) running X11 JSON-RPC 2.0 synthetic input handling (`click`, `move`, `type`, `key`, `drag`) without dropping events, corrected X11 root window plane masking, host-side zero-dependency PNG scanline redaction masking sensitive visual regions, and an asynchronous Python SDK `ComputerTool`.
3. **Microkernel Zero-Trust Security & Storage Encryption**: Strict memory buffer limits on incoming vsock byte streams (`read_stream_with_limit`) truncating rogue payloads without panicking or leaking memory; transparent host-level TLS egress proxy enforcement emitting TLS Fatal Alert 49 on unauthorized domains while passing package registries; AES-256-GCM authenticated persistence for SQLite databases and task ledgers at rest derived from attested SPIFFE identity keys.
4. **SPIRE Identity Federation & Transport Fallback Resilience**: Workload identity verification authenticating federated trust domains and enterprise OIDC tokens against SPIRE federation mappings; resilient MCP transport client detecting UDP/QUIC packet failures within 1 RTT (<100ms) and silently failing over to HTTP/2 over TCP with full in-flight tool call replay.
5. **Dual-Track Verification & Adversarial Hardening**: Independent E2E test track (Tiers 1-4) published via `TEST_READY.md`, followed by Phase 1 100% E2E test passage and Phase 2 Tier 5 adversarial coverage hardening.

## Feature Inventory
| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| 1 | Credential Redaction Formatting | Custom `Display` and `Debug` implementations on `Credentials` producing opaque `[REDACTED-TOKEN-...]` handles | M1 | Survey (R1) |
| 2 | SPIFFE Scope Enforcement | Strict non-escalation authorization checks verifying caller SVID possesses target scopes before minting | M1 | Survey (R1) |
| 3 | Shared Component State | Static thread-safe registry in `tools/ephemeral-tunnel/src/lib.rs` ensuring state continuity across WASM calls | M1 | Survey (R1) |
| 4 | RAII Revocation Drop Guard | Safe cleanup guard in `GitManager::open_pr` guaranteeing credential revocation upon completion, cancellation, or error | M1 | Survey (R1) |
| 5 | Out-of-Band Sandbox Injection | Direct credential injection into `BashSessionHandle.current_env` without exposing secrets to model context | M1 | Survey (R1) |
| 6 | Non-Ambient VCS PR Creation | `GitManager::open_pr` using attested caller SVID, scoped ephemeral token, and clean token lifecycle | M1 | Survey (R1) |
| 7 | X11 Plane Mask Depth Fix | Correct `plane_mask` clamping to `(1 << screen.root_depth) - 1` in `vsock-screenshot-daemon` preventing `BadMatch` errors | M2 | Survey (R2) |
| 8 | Guest Daemon JSON-RPC Protocol | Line-delimited JSON-RPC 2.0 input parser and dispatcher in `vsock-screenshot-daemon` for synthetic UI events | M2 | Survey (R2) |
| 9 | Synthetic Input Execution | Implement `click`, `move`, `drag`, `type`, `key` in `vsock-screenshot-daemon/src/input.rs` via X11 XTest/core protocol | M2 | Survey (R2) |
| 10 | Host-Side PNG Visual Redaction | Zero-dependency pure Python PNG scanline redaction masking bounding boxes with black pixels preserving PNG integrity | M2 | Survey (R2) |
| 11 | Python SDK ComputerTool | Asynchronous `ComputerTool` in `aura-sdk` exposing `click`, `move`, `drag`, `type`, `key`, and redacted screenshot capture | M2 | Survey (R2) |
| 12 | Vsock Port Harmonization | Reconcile screenshot daemon port defaults (5254) across hypervisor, image builder, and daemon | M2 | Survey (R2) |
| 13 | Vsock Stream Buffer Limits | Configurable memory bounds (`VsockStreamConfig`) with instant truncation on buffer limit violations | M3 | Survey (R3) |
| 14 | Egress Proxy Module Re-Export | Expose pure-Rust `EgressProxy` in `aura-core/src/hypervisor/mod.rs` and wire into `FirecrackerVm` lifecycle | M3 | Survey (R3) |
| 15 | AES-256-GCM Encrypted Storage | `EncryptedDatabase` container over `rusqlite` using authenticated AES-256-GCM envelope (`[AURA][VERSION][NONCE][CIPHERTEXT+TAG]`) | M3 | Survey (R3) |
| 16 | Task Ledger at Rest | `TaskLedger` recording task states, transitions, and metadata persisted inside `EncryptedDatabase` | M3 | Survey (R3) |
| 17 | SPIFFE Key Derivation | BLAKE3 KDF key derivation from attested `Svid` for identity-locked database encryption keys | M3 | Survey (R3) |
| 18 | SPIRE Trust Domain Federation | Extend `FederationManager` with SPIRE federation bundle mappings for external trust domains | M4 | Survey (R4) |
| 19 | Enterprise OIDC Token Verification | `OidcProviderConfig` and `verify_oidc_token` verifying enterprise identity tokens without privilege escalation | M4 | Survey (R4) |
| 20 | TCP Fallback Server Wiring | Connect `McpServer::handle_http_request` into the HTTP/2 TCP listener in `aura-core/src/transport/http3.rs` | M4 | Survey (R4) |
| 21 | Resilient Transport Client | `ResilientMcpTransportClient` detecting UDP/QUIC failure within 1 RTT (<100ms) and replaying in-flight requests on TCP | M4 | Survey (R4) |
| 22 | Comprehensive E2E Test Suite | 4-tier opaque-box test suite (Tiers 1-4) validating all inventoried features independently published in `TEST_READY.md` | E2E | Dual Track |
| 23 | Adversarial Coverage Hardening | White-box stress and edge case hardening (Tier 5) with zero gaps | M5_FINAL | Dual Track |

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| E2E | E2E Testing Track | Build 4-tier test suite (Tiers 1-4), harness, and publish TEST_READY.md | none | DONE |
| M1 | Ephemeral Credential Lifecycle & VCS Integration | Features 1-6 (tools/ephemeral-tunnel, git.rs, sandbox injection) | none | DONE |
| M2 | Sandboxed GUI Automation & Visual Redaction | Features 7-12 (vsock-screenshot-daemon, computer.py, redaction) | none | DONE |
| M3 | Microkernel Zero-Trust Security & Storage Encryption | Features 13-17 (vsock buffer limit, egress TLS proxy, AES-256-GCM DB/Ledger) | M1 | IN_PROGRESS |
| M4 | SPIRE Identity Federation & Transport Fallback | Features 18-21 (SPIRE federation, OIDC token verification, 1-RTT TCP failover) | M3 | PLANNED |
| M5_FINAL | Final Milestone: 100% E2E Pass & Adversarial Hardening | Phase 1: 100% pass of Tiers 1-4; Phase 2: Tier 5 adversarial hardening | E2E, M1, M2, M3, M4 | PLANNED |

## Interface Contracts
### `tools/ephemeral-tunnel` ↔ `aura-core/src/knowledge/git.rs`
- `Credentials`: `token: String`, `expires_at: u64`, `service: String`. Custom `Display` & `Debug` outputting `[REDACTED-TOKEN-...]`.
- `EphemeralTunnelService::mint_credentials(&self, service: &str, caller_svid: &Svid) -> Result<Credentials, EphemeralTunnelError>`
- `EphemeralTunnelService::revoke_credentials(&self, token: &str) -> Result<(), EphemeralTunnelError>`
- `GitManager::open_pr(&self, caller_svid: &Svid, title: &str, body: &str) -> Result<String, String>`: Uses RAII auto-revocation drop guard.

### `vsock-screenshot-daemon` ↔ `aura-sdk/aura_sdk/tools/computer.py`
- Protocol: JSON-RPC 2.0 over vsock (port 5254).
- Methods:
  - `{"jsonrpc": "2.0", "id": N, "method": "click", "params": {"x": int, "y": int, "button": int}}` -> `{"jsonrpc": "2.0", "id": N, "result": {"status": "ok"}}`
  - `{"jsonrpc": "2.0", "id": N, "method": "move", "params": {"x": int, "y": int}}` -> `{"jsonrpc": "2.0", "id": N, "result": {"status": "ok"}}`
  - `{"jsonrpc": "2.0", "id": N, "method": "drag", "params": {"x": int, "y": int}}` -> `{"jsonrpc": "2.0", "id": N, "result": {"status": "ok"}}`
  - `{"jsonrpc": "2.0", "id": N, "method": "type", "params": {"text": str}}` -> `{"jsonrpc": "2.0", "id": N, "result": {"status": "ok"}}`
  - `{"jsonrpc": "2.0", "id": N, "method": "key", "params": {"key": str, "action": "press"|"release"}}` -> `{"jsonrpc": "2.0", "id": N, "result": {"status": "ok"}}`
  - `{"jsonrpc": "2.0", "id": N, "method": "screenshot", "params": {}}` -> `{"jsonrpc": "2.0", "id": N, "result": {"png_base64": str}}`
- Redaction Filter: `mask_image_bytes(png_bytes: bytes, bounding_boxes: List[Tuple[int, int, int, int]]) -> bytes`

### `aura-core/src/hypervisor` ↔ `aura-core/src/storage`
- `VsockStreamConfig { max_buffer_bytes: usize }`: `read_stream_with_limit<R: AsyncRead + Unpin>(&mut R, max_bytes: usize) -> Result<Vec<u8>, VsockStreamError>` truncates immediately on limit violation.
- `EgressProxy`: Transparent TLS proxy enforcing allow-listed domain SNIs and issuing TLS Alert 49 (`0x15, 0x03, 0x03, 0x00, 0x02, 0x02, 0x31`) on violations.
- `EncryptedDatabase`: `open<P: AsRef<Path>>(path: P, key: [u8; 32]) -> Result<Self, StorageError>`. Schema and records encrypted with AES-256-GCM.
- `TaskLedger`: `append_event(&self, event: &TaskEvent) -> Result<(), StorageError>` and `get_events(&self, task_id: &str) -> Result<Vec<TaskEvent>, StorageError>`.

### `aura-core/src/identity` ↔ `aura-core/src/transport`
- `SpireFederationMapping`: Map federated trust domains and enterprise OIDC claims to bounded SPIFFE SVID capabilities without privilege escalation.
- `ResilientMcpTransportClient`: Primary HTTP/3 over QUIC/UDP with 1-RTT timeout fallback (<100ms) to HTTP/2 over TCP with in-flight retry.

## Code Layout
- `tools/ephemeral-tunnel/`: Ephemeral tunnel CLI, library, and WASM component.
  - `src/credentials.rs`: Redacted Display/Debug formatting.
  - `src/spire.rs`: Ephemeral tunnel service with SPIFFE scope validation.
  - `src/lib.rs`: Static shared service state across component calls.
- `deploy/images/vsock-screenshot-daemon/`:
  - `src/capture.rs`: Fixed X11 depth mask calculation.
  - `src/input.rs`: X11 synthetic input actions (`click`, `move`, `drag`, `type`, `key`).
  - `src/server.rs`: JSON-RPC 2.0 vsock server loop.
- `aura-core/`:
  - `src/hypervisor/firecracker.rs`: Vsock stream buffer truncation and port reconciliation.
  - `src/hypervisor/proxy.rs` & `src/hypervisor/mod.rs`: Egress proxy re-export and lifecycle.
  - `src/knowledge/git.rs`: `GitManager::open_pr` with caller SVID validation and RAII revocation guard.
  - `src/storage/`: New encrypted storage module (`encrypted.rs`, `ledger.rs`).
  - `src/identity/`: Federation mappings and OIDC token verification.
  - `src/transport/`: HTTP/2 TCP fallback server and resilient client.
- `aura-sdk/`:
  - `aura_sdk/tools/computer.py`: Async ComputerTool with pure Python PNG scanline visual redaction.
  - `tests/test_computer_tool.py`: Full unit and integration test coverage for ComputerTool and redaction.
