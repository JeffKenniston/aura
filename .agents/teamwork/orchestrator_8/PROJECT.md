# Project: Aura M0-M3 Gap Implementation

## Architecture
- **Runtime & Concurrency (M1)**: Multi-threaded Tokio runtime pool (`aura-core/src/runtime/mod.rs`), blocking task pool isolation, runtime metrics (`RuntimeMetrics` via `tokio_unstable`), and 5k stream QUIC load testing (`aura-core/tests/http3_stream_concurrency_test.rs`). [STATUS: PASSED]
- **Declarative Config & MCP HTTP/3 (M2)**: Serde TOML manifest loading for `Registry` (`aura-core/src/runtime/registry.rs`), stateless JSON-RPC 2.0 MCP server over HTTP/3 (`aura-core/src/transport/mcp.rs`, `aura-core/src/transport/mod.rs`). [STATUS: PASSED]
- **Authentic Task Execution & MicroVM Sandboxing (M3)**: Dynamic SVID capability enforcement and real WASM/Firecracker task dispatch in Zenoh (`aura-core/src/bus/mod.rs`), `/dev/kvm` nested virtualization validation and error propagation (`aura-core/src/hypervisor/firecracker.rs`, `aura-core/src/hypervisor/mod.rs`), immutable guest rootfs with ephemeral scratch disk layering (`aura-core/src/hypervisor/firecracker.rs`). [STATUS: PASSED]
- **Persistent Stateful Tools & GAP-003 (M4)**: In-guest persistent shell daemon (`deploy/images/vsock-shell-daemon`), vsock shell session management (`aura-core/src/hypervisor/firecracker.rs`, `aura-sdk/aura_sdk/tools/bash.py`), host-to-guest vsock TCP forwarding for Chromium CDP port 9222 and async Python CDP client (`aura-core/src/hypervisor/firecracker.rs`, `aura-sdk/aura_sdk/tools/browser.py`), formal documentation and static WIT mitigation tracking for GAP-003 (`aura-core/src/hypervisor/wasm.rs`, `docs/architecture/adrs/ADR-001-Execution-Host.md`, `references/02-execution-sandboxes.md`). [STATUS: IN_PROGRESS]
- **Multi-Agent Verification (M5)**: End-to-end multi-agent verification gate across Rust microkernel, Python SDK, sandboxed tools, and forensic integrity auditing. [STATUS: PLANNED]

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| 1 | M1: Runtime & Concurrency | M0-SPIKE-03, HOST-002, HOST-004 | none | PASSED |
| 2 | M2: Declarative Config & MCP HTTP/3 | HOST-011, NET-007 | M1 | PASSED |
| 3 | M3: Authentic Task Execution & MicroVM Sandboxing | BUS-003, EXE-VM-002, EXE-VM-011 | M2 | PASSED |
| 4 | M4: Persistent Stateful Tools & GAP-003 | TOOL-BASH-002, TOOL-BRW-003, GAP-003 | M3 | IN_PROGRESS |
| 5 | M5: Final Multi-Agent Verification Gate & Delivery | Full integration, multi-agent review, victory audit | M4 | PLANNED |
