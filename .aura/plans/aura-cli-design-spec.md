# Aura CLI (aura-cli) Architectural Implementation Specification

## 1. Architecture Discovery
**Module Ownership:** Terminal Layer (`aura-cli`)
**Language:** Rust
**Invariants Enforced:**
- **Zero FFI:** Complete decoupling from Python substrate (`aura-sdk`). Communication exclusively via Zenoh 1.5+ pub/sub bus.
- **Docker-Free Execution:** Heavyweight tasks orchestrated via Firecracker microVMs; lightweight tasks via WASM (WASI 0.3 Component Model).
- **Zero-Trust Identity:** Strict enforcement of SPIFFE/SPIRE mTLS attestation and Cedar-policy authorization.

## 2. AST Blast-Radius Pre-Check
*Note: The `.agents/knowledge_graph.sqlite` database is not yet initialized for this repository. The blast radius for a new CLI implementation is effectively 0 upstream callers. We will proceed with the architectural specification.*

## 3. Substrate Selection
- **TUI & Event Loop:** Native Rust executable running via `tokio` multi-threaded runtime.
- **Heavyweight Tool Execution:** Firecracker microVMs (managed by `aura-core`, bridged via AF_VSOCK to UnixStream).
- **Lightweight Background Tasks (Token counting, compression):** WebAssembly (WASI 0.3) compiled components, leveraging native async futures.

## 4. Phased Architectural Blueprint Roadmap

### Phase 1: Foundation & Bootstrapping
- **Objective:** Establish the `aura-cli` Rust crate targeting both `wasm32-wasip2` and native triples.
- **Dependencies:** `tokio`, `ratatui`, `crossterm`, `zenoh`, `spiffe`, `cedar-policy`.
- **Deliverables:**
  - Initialize Centralized `AppModel` adhering to The Elm Architecture (TEA).
  - Define immutable state structures for chat history, artifact tracking, and system metrics.

### Phase 2: Asynchronous Event Loop & TUI Rendering
- **Objective:** Implement non-blocking UI interactions and rendering.
- **Implementation:**
  - Utilize `tokio::select!` multiplexing for three asynchronous streams:
    1. Terminal Events (`crossterm::event::EventStream`)
    2. Zenoh Subscriptions
    3. WASI 0.3 Background Task `mpsc` channels
  - Implement double-buffered `ratatui` rendering pipeline for microsecond frame updates.
  - Integrate `miette` for stylized error tracing and `ratatui-image` for rich media rendering.

### Phase 3: Zero-Copy IPC Pipeline
- **Objective:** Eliminate serialization overhead using Zenoh 1.5+ Shared Memory (SHM).
- **Implementation:**
  - Configure `z_shm_provider` for shared memory allocations.
  - Implement Semantic Namespace Routing (e.g., `aura/repository/index?workspace=./current`).
  - Develop the Zero-Copy Artifact Review pane, casting `ZBytes` references to `#[repr(C)]` layouts using `TypedLayout::new()`.

### Phase 4: Multi-Tiered Security & Attestation
- **Objective:** Enforce zero-trust principles at transport and application layers.
- **Implementation:**
  - **Layer 1 (Transport):** Integrate `spiffe-rustls-tokio` for dynamic X.509 SVID acquisition and mTLS Zenoh sessions.
  - **Layer 2 (Application):** Embed `cedar-policy` to evaluate execution modes (`strict`, `request-review`, `proceed-in-sandbox`, `always-proceed`).
  - Implement interactive verification blocks for `ToolCallRequest` events requiring human approval.

### Phase 5: Headless Pipelines & MicroVM Bridging
- **Objective:** Support subagent delegation and isolated execution observability.
- **Implementation:**
  - Develop `--headless` mode for integration with CI/CD and programmatic `aura-sdk` invocations.
  - Construct custom tower services to map `tokio::net::UnixStream` proxy events (bridged from AF_VSOCK) into real-time standard output streams in the TUI.

## 5. Verification Gates
1. **Compilation Check:** Crate must compile successfully for both `x86_64-unknown-linux-gnu` and `wasm32-wasip2`.
2. **Attestation Gate:** `aura-cli` must successfully authenticate with a local SPIRE agent before establishing a Zenoh connection.
3. **IPC Benchmark:** SHM payload transfer from `aura-core` mock to `aura-cli` must demonstrate <1ms latency for 10MB structured payloads.

## 6. Rollback Plan
- Revert Git commits to the pre-initialization state.
- Disconnect `aura-cli` identities from the local SPIRE server.
- Terminate lingering Zenoh daemon processes associated with the TUI.
