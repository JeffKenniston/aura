# Aura

**Aura** is a decoupled, local-first multi-agent orchestration operating system engineered for autonomous software engineering.

## Architectural Paradigms & Core Invariants

Aura is built on the following core principles:

1. **Decoupled Language Boundaries (Zero FFI)**:
   - **Execution Host (Rust - `aura-core`)**: Manages hypervisor control, network transport, memory isolation, and cryptographic attestation using an asynchronous Tokio runtime.
   - **Cognitive Scripting (Python - `aura-sdk`)**: Substrate for agent cognitive behaviors, heuristic routing, and AI framework integration.
   - **Communication Protocol**: Strictly event-driven publish/subscribe messaging over the **Zenoh** bus. Direct Foreign Function Interfaces (FFI) between Rust and Python are strictly prohibited.

2. **Docker-Free Bifurcated Sandboxing**:
   - **Lightweight Workloads**: Executed via WebAssembly (WASI 0.3 Component Model) hosted by `wasmtime`.
   - **Heavyweight Workloads**: Executed inside ephemeral Firecracker microVMs over the Linux KVM API, isolated under secondary `jailer` daemons.

3. **Zero-Trust Cryptographic Identity (SPIFFE/SPIRE)**:
   - All active agents, WASM components, and microVM sandboxes must hold a dynamically issued SPIFFE Verifiable Identity Document (SVID).

4. **Model Context Protocol (MCP) over HTTP/3**:
   - Tool and context transport implements the MCP specification natively over HTTP/3.

5. **Cognitive Processing & State Management**:
   - LLM cognitive sessions interface via the Google Gemini Interactions API using server-side caching.
   - Dynamic Multi-Tier Model Routing (Gemini 3.1 Pro, 3.8 Flash, 3.5 Flash-Lite, Deep Research).

6. **Semantic Code Intelligence**:
   - Source code analysis utilizes Concrete Syntax Trees (CST) generated via `tree-sitter`.
   - Incremental indexing relies on BLAKE3 Merkle tree hashing via `rayon` feeding an SQLite database with hybrid BM25 and vector embeddings.

## Project Structure

- `aura-core/`: Rust execution host and hypervisor manager.
- `aura-sdk/`: Python cognitive scripting and AI framework integration.
- `aura-cli/`: Command-line interface for Aura.
- `.agents/`: Agent configuration and multi-step execution workflows.
- `.aura/`: Workspace configuration for Aura.

## License

This project is proprietary. All rights reserved. Please see the `LICENSE` file for more information.
