# AGENTS.md — Foundational Repository Constitution for Aura

## 1. Architectural Paradigms & Core Invariants

Aura is a decoupled, local-first multi-agent orchestration operating system engineered for autonomous software engineering. All agents, tools, and processes operating in this workspace must strictly adhere to the following universal invariants:

1. **Decoupled Language Boundaries (Zero FFI)**:
   - **Execution Host (Rust - `aura-core`)**: Manages hypervisor control, network transport, memory isolation, and cryptographic attestation using an asynchronous Tokio runtime.
   - **Cognitive Scripting (Python - `aura-sdk`)**: Substrate for agent cognitive behaviors, heuristic routing, and AI framework integration.
   - **Communication Protocol**: Strictly event-driven publish/subscribe messaging over the **Zenoh** bus. Direct Foreign Function Interfaces (FFI) between Rust and Python are strictly prohibited to isolate the core kernel from language-level crashes or segmentation faults.

2. **Docker-Free Bifurcated Sandboxing**:
   - Traditional container runtimes (e.g., Docker, OCI containers) are strictly prohibited.
   - **Lightweight Workloads**: Executed via WebAssembly (WASI 0.3 Component Model) hosted by `wasmtime` with Cranelift JIT compilation, pooling allocators, and strict instruction fuel-metering.
   - **Heavyweight Workloads**: Executed inside ephemeral Firecracker microVMs over the Linux KVM API (<125ms boot, <5MiB memory overhead), isolated under secondary `jailer` daemons enforcing cgroups and seccomp system-call filtering. MicroVMs are terminated immediately post-execution.

3. **Zero-Trust Cryptographic Identity (SPIFFE/SPIRE)**:
   - All active agents, WASM components, and microVM sandboxes must hold a dynamically issued SPIFFE Verifiable Identity Document (SVID).
   - No capability or tool (such as filesystem access or network tunneling) may be executed without presenting a cryptographically signed SVID explicitly authorized for that scope.

4. **Model Context Protocol (MCP) over HTTP/3**:
   - Tool and context transport implements the July 2026 stateless MCP specification natively over HTTP/3 (QUIC via `tokio-quiche`).
   - Transport resilience relies on independent QUIC byte streams to eliminate Head-of-Line (HOL) blocking.
   - Mandatory silent fallback: Enterprise UDP blocking on port 443 must trigger immediate degradation to HTTP/2 over TCP within 1 RTT via Alt-Svc / ALPN negotiation.

5. **Cognitive Processing & State Management**:
   - LLM cognitive sessions interface via the Google Gemini Interactions API using server-side caching (`previous_interaction_id`) to optimize time-to-first-token (TTFT) and preserve token budget.
   - Dynamic Multi-Tier Model Routing:
     - **Gemini 3.1 Pro**: Reserved for complex multi-file architectural planning, deep reasoning, and high blast-radius refactoring (Tier 3).
     - **Gemini 3.8 Flash**: Default orchestration model for feature implementation, tool orchestration, and long-horizon tasks.
     - **Gemini 3.5 Flash-Lite**: High-throughput validation, subagent verification, AST parsing, and log evaluation.
     - **Deep Research**: Pre-processing data collection and comprehensive technical synthesis.

6. **Semantic Code Intelligence**:
   - Source code analysis utilizes Concrete Syntax Trees (CST) generated via `tree-sitter` to preserve whitespace, punctuation, and comments during AST transformation.
   - Incremental indexing relies on BLAKE3 Merkle tree hashing via `rayon` feeding an SQLite database with hybrid BM25 and vector embeddings.

---

## 2. Primary Verification Gates

Before any architectural change, refactoring, or implementation is applied, the agent must pass through the following deterministic gates:

1. **AST Blast-Radius Analysis**: Recursive Common Table Expression (CTE) dependency tracing must evaluate all affected callers and reverse-dependencies. If the blast radius exceeds threshold tolerances, changes must be escalated to Gemini 3.1 Pro and require human sign-off.
2. **Deterministic Tool Hooks**: All code formatters, linters, and type checkers (`cargo clippy`, `cargo fmt --check`, `ruff`, `mypy`) run via local binaries triggered through `PostToolUse` lifecycle hooks rather than language model self-evaluation.
3. **Sandbox Attestation Check**: Prior to tool execution, verify that target sandboxes (Firecracker microVM or WASI component) have active jailer policies and valid SPIFFE SVIDs.

---

## 3. Operational Protocols & Guardrails

1. **The Three-Round Rule (Circuit Breaker)**:
   - If an operation, build, or test fails to resolve within **three iterations**, the agent MUST immediately halt its autonomous loop.
   - The agent is forbidden from attempting speculative permutations. It must pause and present a concise diagnostic summary and clarifying question to the developer.

2. **Slash Command Conventions**:
   - `/plan`: Mandatory command before executing multi-file modifications. Requires drafting a structured implementation specification with risk assessment and verification criteria.
   - `/grill-me`: Triggers an active interrogation session where the agent actively questions the developer regarding ambiguous requirements, failure modes, and architectural trade-offs.
   - `/boost`: Multi-agent orchestration command reserved strictly for complex Tier 3 tasks requiring parallel subagent swarms. Prohibited for routine operations to prevent token exhaustion.

3. **Context Hygiene**:
   - Large configurations, multi-step execution workflows, and directory-specific constraints are strictly modularized into `.agents/rules/`, `.agents/skills/`, and `.agents/agents/` to prevent context saturation.
   - Heavy directories (`target/`, `.venv/`, `node_modules/`, `rootfs/`, `*.sqlite`) are ignored via `.ignore` and `.cursorignore`.

---

## 4. Domain Vocabulary

- **Aura Microkernel**: The core Rust execution host supervising WASM runtimes, microVM lifecycles, and QUIC/HTTP3 transport.
- **Zenoh Bus**: Zero-copy, publish/subscribe event routing layer linking Rust host and Python cognitive scripts.
- **WASI 0.3**: WebAssembly System Interface Component Model featuring native asynchronous streams and futures.
- **Firecracker / Jailer**: Lightweight KVM-based microVM runner and isolation wrapper enforcing cgroups and seccomp filters.
- **SVID**: SPIFFE Verifiable Identity Document used for mutual zero-trust authentication between system components.
- **Interactions API**: Gemini server-side stateful session API with server-sent event (SSE) streaming and background execution.
- **AST Blast-Radius**: CTE-based dependency evaluation mechanism measuring systemic risk of code modifications.

---

## 5. WSL Sudo Password

- **Password**: "1029123"