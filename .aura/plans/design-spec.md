# Aura CLI Architectural Design Specification: Phase 2

## 1. Overview
This specification delineates Phase 2 of the `aura-cli` architecture, focusing exclusively on establishing a high-performance, zero-copy Inter-Process Communication (IPC) layer. It replaces traditional socket-based IPC by integrating Eclipse Zenoh (≥ 1.5) Shared Memory (SHM), enabling deterministic sub-millisecond data transmission between the `aura-cli` frontend and the `aura-core` execution microkernel.

## 2. Phase Breakdown and Module Boundaries

### Phase 2.1: Zenoh Integration
- **Objective:** Eliminate TCP serialization overhead and traditional UNIX domain sockets.
- **Modules (`src/ipc/zenoh.rs`):**
  - **Shared Memory Topological Bus:** Integrate `zenoh` and `zenoh-shm` dependencies to instantiate the client node.
  - **Connection Topology:** Connect the CLI directly to the `aura-core` execution microkernel via Zenoh's publish/subscribe topological bus rather than point-to-point sockets.

### Phase 2.2: Semantic Key Routing
- **Objective:** Establish a canonical namespace for deterministic event and state routing.
- **Key Expressions:**
  - `aura/repository/index?workspace={path}`: Used for broadcasting context ingestion and workspace index updates.
  - `aura/core/agent/{session_id}/stream`: Used to subscribe to high-frequency execution telemetry and token emissions from active model sessions.
  - `aura/sessions/{session_id}/approval`: Used for authorizing Cedar policy interception prompts.

### Phase 2.3: Shared Memory Deserialization
- **Objective:** Enable zero-copy memory access for large payloads (> 4KB).
- **Memory Layout (`src/ipc/layout.rs`):**
  - **C-Aligned Structs:** Define shared payload boundaries using `#[repr(C)]` data structures to ensure ABI compatibility across the Zenoh bus.
  - **Typed SHM Buffers:** Implement Zenoh `TypedLayout` mappings to cast memory pointers directly into native Rust structures.
  - **Zero Deserialization:** Bypass serde/JSON serialization entirely for high-bandwidth payloads such as unified code diffs and Concrete Syntax Tree (CST) AST graphs.

## 3. Substrate Evaluation
- **Execution Substrate:** Pure Rust compiled binary (`aura-cli`). Memory mapping occurs natively within the OS virtual memory manager via Zenoh's SHM provider.
- **Model Routing Tier:** Gemini 3.1 Pro (Tier 3 architectural design phase).

## 4. Invariants Verified
- **Zero FFI:** No dynamic Foreign Function Interfaces (C-FFI, PyO3) are required to interoperate with the Python `aura-sdk`; Python nodes access the same Zenoh key expressions, preserving Rust's memory safety.
- **Zero-Copy Boundary:** Memory is mapped directly into `aura-cli` without heap reallocations or `memcpy` calls.
- **SPIFFE Validation Layer:** The underlying Zenoh session must continue enforcing mTLS attestation via `spiffe-rustls-tokio`.

## 5. Verification Gates
1. **Gate 1: Zero-Copy Memory Benchmark:** (Previously stubbed in `benches/shm_zero_copy.rs`). Must execute and transmit a 50MB code diff payload. The heap delta process metrics must register ≤ 512KB growth.
2. **Key Routing Assertion:** Subscriptions to `aura/core/agent/+/stream` must correctly route wildcard topologies to the TEA event multiplexer.
3. **TypedLayout Mapping Validation:** A C-aligned struct injected by `aura-core` must be losslessly cast into `aura-cli` via `zenoh-shm` without undefined behavior or segmentation faults.

## 6. Rollback Strategy
- Modifications will be isolated to the `feature/cli-ipc` branch.
- If `zenoh-shm` allocation triggers permission errors under strict WSL2 or Linux namespaces, we will fall back to loopback UDP Zenoh routing as a temporary circuit breaker while preserving the semantic key routing abstractions.
