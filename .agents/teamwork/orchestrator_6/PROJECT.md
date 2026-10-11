# Project: Aura M0-M3 Gap Implementation

## Architecture
- **Runtime & Concurrency (M1)**: Multi-threaded Tokio runtime pool (`aura-core/src/runtime/mod.rs`), blocking task pool isolation, runtime metrics (`RuntimeMetrics` via `tokio_unstable`), and 5k stream QUIC load testing (`aura-core/tests/http3_stream_concurrency_test.rs`).
- **Declarative Config & MCP HTTP/3 (M2)**: Serde TOML manifest loading for `Registry` (`aura-core/src/runtime/registry.rs`), stateless JSON-RPC 2.0 MCP server over HTTP/3 (`aura-core/src/transport/mcp.rs`, `aura-core/src/transport/http3.rs`).
- **Authentic Task Execution & MicroVM Sandboxing (M3)**: Dynamic SVID capability enforcement and real WASM/Firecracker task dispatch in Zenoh (`aura-core/src/bus/mod.rs`), `/dev/kvm` nested virtualization validation and error propagation (`aura-core/src/hypervisor/firecracker.rs`, `aura-core/src/hypervisor/mod.rs`), immutable guest rootfs with ephemeral scratch disk layering (`aura-core/src/hypervisor/firecracker.rs`).
- **Persistent Stateful Tools & GAP-003 (M4)**: In-guest persistent shell daemon (`deploy/images/vsock-shell-daemon`), vsock shell session management (`aura-core/src/hypervisor/firecracker.rs`, `aura-sdk/aura_sdk/tools/bash.py`), host-to-guest vsock TCP forwarding for Chromium CDP port 9222 and async Python CDP client (`aura-core/src/hypervisor/firecracker.rs`, `aura-sdk/aura_sdk/tools/browser.py`), formal documentation and static WIT mitigation tracking for GAP-003 (`aura-core/src/hypervisor/wasm.rs`, `docs/architecture/adrs/ADR-001-Execution-Host.md`, `references/02-execution-sandboxes.md`).
- **Multi-Agent Verification (M5)**: End-to-end multi-agent verification gate across Rust microkernel, Python SDK, sandboxed tools, and forensic integrity auditing.

## Feature Inventory
| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| 1 | M0-SPIKE-03 | Dedicated Tokio multi-threaded runtime pool builder with configurable worker threads | M1 | Audit Report §1 |
| 2 | HOST-002 | Asynchronous concurrency runtime pool, separate blocking task pool, RuntimeMetrics | M1 | Audit Report §2 |
| 3 | HOST-004 | High-concurrency QUIC stream multiplexing empirical test asserting 5,000+ concurrent streams | M1 | Audit Report §3 |
| 4 | HOST-011 | Declarative Component Registry TOML manifest deserialization and validation | M2 | Audit Report §4 |
| 5 | NET-007 | Stateless HTTP/3 Model Context Protocol (MCP) server interface handling `tools/list` and `tools/call` | M2 | Audit Report §6 |
| 6 | BUS-003 | Authentic Zenoh task queue dispatch executing in real WASM/Firecracker with dynamic SVID | M3 | Audit Report §5 |
| 7 | EXE-VM-002 | Host `/dev/kvm` nested virtualization availability & permission validation | M3 | Audit Report §7 |
| 8 | EXE-VM-011 | MicroVM immutable rootfs (`is_read_only: true`) and ephemeral scratch disk layering | M3 | Audit Report §8 |
| 9 | TOOL-BASH-002 | In-guest `vsock-shell-daemon`, host session manager, and Python SDK session state continuity | M4 | Audit Report §9 |
| 10 | TOOL-BRW-003 | Host-to-guest vsock TCP proxy for Chromium CDP port 9222 and async Python CDP client | M4 | Audit Report §10 |
| 11 | GAP-003 | Formal architectural documentation of WASI 0.3 dynamic linking limitation & static WIT mitigation | M4 | Audit Report §11 |

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| 1 | M1: Runtime & Concurrency | M0-SPIKE-03, HOST-002, HOST-004 | none | IN_PROGRESS |
| 2 | M2: Declarative Config & MCP HTTP/3 | HOST-011, NET-007 | M1 | PLANNED |
| 3 | M3: Authentic Task Execution & MicroVM Sandboxing | BUS-003, EXE-VM-002, EXE-VM-011 | M2 | PLANNED |
| 4 | M4: Persistent Stateful Tools & GAP-003 | TOOL-BASH-002, TOOL-BRW-003, GAP-003 | M3 | PLANNED |
| 5 | M5: Final Multi-Agent Verification Gate & Delivery | Full integration, multi-agent review, victory audit | M4 | PLANNED |

## Interface Contracts
### Runtime & Concurrency (M1)
- `runtime::init_tokio_pool(worker_threads: Option<usize>) -> Result<tokio::runtime::Runtime, Box<dyn std::error::Error>>`
- `runtime::metrics() -> Option<tokio::runtime::RuntimeMetrics>`
- `http3_stream_concurrency_test.rs`: 5,000 concurrent QUIC streams against local listener without thread starvation.

### Declarative Config & MCP HTTP/3 (M2)
- `Registry::load_from_config<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>>`
- `Registry::load_from_str(raw: &str) -> Result<Self, Box<dyn std::error::Error>>`
- `McpHttp3Handler`: JSON-RPC 2.0 `tools/list` and `tools/call`.

### Authentic Task Execution & MicroVM Sandboxing (M3)
- `ZenohBus::start_task_listener_with_registry(&self, registry: Arc<Registry>)`
- `TaskPayload` with optional `payload`, `svid`, `fuel_budget`.
- `TaskResult` with `stdout`, `stderr`, `exit_code`, `duration_ms`.
- `firecracker::validate_kvm_support() -> Result<(), KvmError>`
- `VmConfig::is_read_only_rootfs: bool`, `VmConfig::scratch_disk_path: Option<String>`

### Persistent Stateful Tools & GAP-003 (M4)
- `BashSessionHandle`, `BashSessionManager` in `firecracker.rs`.
- `CdpVsockForwarder` proxying loopback TCP to Firecracker vsock port 9222.
- `BashTool.execute(command: str, session_id: str | None = None) -> str`
- `AsyncCdpClient` and `BrowserTool` methods (`navigate`, `click`, `evaluate`, `screenshot`, `close`).
- GAP-003 architectural acknowledgment in `wasm.rs`, `ADR-001`, and `02-execution-sandboxes.md`.

## Code Layout
- `aura-core/src/runtime/mod.rs`
- `aura-core/src/runtime/registry.rs`
- `aura-core/src/transport/http3.rs`
- `aura-core/src/transport/mcp.rs`
- `aura-core/src/bus/mod.rs`
- `aura-core/src/hypervisor/mod.rs`
- `aura-core/src/hypervisor/firecracker.rs`
- `aura-core/src/hypervisor/wasm.rs`
- `aura-core/tests/http3_stream_concurrency_test.rs`
- `deploy/images/vsock-shell-daemon/`
- `aura-sdk/aura_sdk/tools/bash.py`
- `aura-sdk/aura_sdk/tools/browser.py`
- `aura-sdk/tests/test_bash_tool.py`
- `aura-sdk/tests/test_browser_tool.py`
- `docs/architecture/adrs/ADR-001-Execution-Host.md`
- `.agents/plugins/aura-dev-kit/skills/aura-architecture/references/02-execution-sandboxes.md`
