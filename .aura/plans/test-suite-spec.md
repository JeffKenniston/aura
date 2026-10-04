# Aura Framework End-to-End Test Suite Specification

## 1. Architecture Discovery & Module Ownership

The Aura Framework encompasses the following core modules:
- **`aura-core` (Rust)**: The execution host. Manages the Tokio asynchronous runtime, WebAssembly (WASI 0.3) component model execution, Firecracker microVM lifecycle, Zenoh pub/sub networking, HTTP/3 transport, and SPIFFE/SPIRE cryptographic identity.
- **`aura-sdk` (Python)**: The cognitive scripting layer. Implements agent behaviors, manages API interactions (Gemini Models), handles heuristic routing, and interfaces with the underlying host strictly via Zenoh bus.
- **`aura-cli` (Rust)**: The command-line developer entry point.
- **`tools/` & `sandboxes/`**: WASI and Firecracker workload execution boundaries.

**Invariants Checked:**
- WebAssembly (WASI 0.3) used for lightweight sandboxing; Firecracker for heavyweight sandboxing.
- Strictly decoupled via Zenoh bus (zero direct Python-Rust FFI).
- Required integration with SPIFFE identity attestation for all components.

## 2. AST Blast-Radius Pre-Check

- **Target Symbols:** Creation of new unit and integration tests across all directories. No modification of core APIs or dependencies.
- **Query Results:** `0` upstream callers affected directly by the addition of tests.
- **Blast Radius:** Low (`0` < 10 threshold). Escalate to Gemini 3.1 Pro (Already Active).

## 3. Substrate Selection

- **Role:** Testing framework and orchestration.
- **Substrate:** Test suites will execute within their respective native toolchains (Cargo, Pytest), simulating both lightweight WASI interactions and heavyweight Firecracker isolation contexts. Integration tests will orchestrate the Rust Microkernel bridging to the Python SDK over local Zenoh bus sockets.

## 4. Implementation Specification

### 4.1 Unit Testing (`tests/unit/`)
- **Rust `aura-core`:** Test Tokio QUIC initialization, SVID extraction & validation, WebAssembly component loading, and Zenoh publisher mocking.
- **Python `aura-sdk`:** Test model routing logic, caching mechanisms, prompt generation, and Zenoh subscriber callbacks using `pytest` and `anyio`.

### 4.2 Integration Testing (`tests/integration/`)
- **End-to-End Communication:** Spin up `aura-core` backend, subscribe from `aura-sdk`, and test message latency, format validation, and resilience over Zenoh.
- **Model Interaction Simulation:** Verify cognitive loops and tool hook parsing without relying on external APIs (mocked Gemini Interaction endpoints).

### 4.3 Sandbox Attestation & Execution (`tests/sandboxes/`)
- **WASI Sandboxing:** Execute memory-bound and fuel-metered WASI workloads via `aura-core` to verify hard limits are enforced and execution drops correctly upon exhaustion.
- **Firecracker Isolation:** Execute a dummy tool hook inside a simulated microVM, ensuring <125ms boot and immediate teardown upon process exit.
- **Identity Enforcement:** Simulate missing SPIFFE SVIDs and test boundary rejections.

## 5. Verification Gates

1. Run formatting tools: `cargo fmt --check`, `cargo clippy`, `ruff check`, `mypy`.
2. Ensure test additions do not pollute the core modules with direct FFI dependencies or circular imports.
3. Validate that standard `cargo test` and `pytest` invocations succeed.

## 6. Rollback Plan

- Remove added test directories and files.
- Revert any configuration file changes made (e.g., `Cargo.toml`, `pytest.ini`).

---
**Status:** Waiting for developer sign-off to proceed with test implementation.
