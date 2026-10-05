# AGENTS.md — Foundational Repository Constitution for Aura & Aura CLI

## 1. Architectural Paradigms & Core Invariants

Aura is a decoupled, local-first multi-agent orchestration operating system engineered for autonomous software engineering. The platform enforces a trifurcated architecture separating presentation, kernel execution, and cognitive scripting:
- **Terminal Interface (`aura-cli`)**: High-performance, pure Rust keyboard-driven presentation node and inter-process communication (IPC) client.
- **Execution Microkernel (`aura-core`)**: Rust host managing Firecracker microVMs, WASI 0.3 runtimes, and HTTP/3 MCP transport.
- **Cognitive Substrate (`aura-sdk`)**: Python environment providing declarative agent topologies and heuristic loops without exposing `aura-core` to interpreter crashes or FFI memory risks.

All agents, tools, and processes operating in this workspace must strictly adhere to the following universal invariants:

1. **Decoupled Language Boundaries (Zero FFI)**:
   - Direct Foreign Function Interfaces (C-FFI, PyO3, ctypes) between Rust components and Python agents are strictly prohibited.
   - All inter-process communication proceeds across an asynchronous, event-driven publish/subscribe data bus powered by **Eclipse Zenoh 1.5**.

2. **Aura CLI (aura-cli) Presentation Architecture**:
   - **Stateless Presentation Layer**: `aura-cli` contains no direct Large Language Model (LLM) network stacks, no heavy sandbox orchestration logic, and no persistent database drivers.
   - **Asynchronous TEA Concurrency**: Built on an asynchronous adaptation of The Elm Architecture (TEA). The visual output is a pure function of an immutable state model (`AppModel`) driven by a central `tokio::select!` event loop multiplexing:
     1. User keyboard & terminal signals via `crossterm`.
     2. Zenoh Shared Memory (`zenoh-shm`) kernel messages.
     3. Native WASI 0.3 async telemetry channels.
   - **Immediate-Mode Double Buffering**: Renders at up to 60 FPS using `ratatui` double-buffering. Inline visual assets render via Sixel, Kitty Graphics Protocol, or iTerm2 formats using `ratatui-image`.
   - **Zero-Copy Shared Memory IPC**: Inter-process communication between `aura-cli` and `aura-core` uses Zenoh Shared Memory (`zenoh-shm`) with `#[repr(C)]` layouts and `TypedLayout` zero-deserialization mapping for payloads >4KB (AST graphs, unified diffs).
   - **Canonical Zenoh Key Expressions**:
     - Context Ingestion: `aura/repository/index?workspace={path}`
     - Execution Telemetry: `aura/core/agent/{session_id}/stream`
     - Authorization Pipeline: `aura/sessions/{session_id}/approval`
     - Tool Invocation Hooks: `aura/tools/{tool_name}/exec`

3. **Two-Layer Zero-Trust Security & Progressive Authorization**:
   - **Layer 1 (Transport Attestation)**: Dynamic X.509 SVID attestation via SPIFFE/SPIRE (`spiffe-rustls-tokio`) establishing mutual TLS (mTLS) over the Zenoh bus (`spiffe://aura.local/workload/aura-cli`). Unauthenticated connections are dropped immediately.
   - **Layer 2 (Progressive Authorization)**: Granular policy evaluation via embedded **Cedar Policy Engine** (`cedar-policy`, `cedarling`). Evaluates four permission tiers:
     - `strict`: Zero-trust read-only enforcement; all mutations require terminal confirmation.
     - `request-review`: Default mode; autonomous planning, search, and diff generation; halts execution at mutation diffs requiring explicit human review.
     - `proceed-in-sandbox`: Autonomous tool execution routed into isolated Firecracker microVMs or WASI engines without host prompts.
     - `always-proceed`: Direct host mutation; restricted to automated headless CI/CD environments.

4. **MicroVM Telemetry & WASI 0.3 Integration**:
   - Heavyweight tool executions in Firecracker microVMs stream terminal standard output over VirtIO-vsock (`AF_VSOCK`) kernel sockets, bridged by `tokio::net::UnixStream` and `tower`/`hyper` into Zenoh pub/sub.
   - Lightweight tools run via `wasmtime` (version >= 47) using the WASI 0.3 Component Model with async streams and futures in the ABI.

5. **Headless Subagent Delegation (`aura-cli -p`)**:
   - Supports headless non-interactive subagent execution (`aura-cli --headless -p "..."`) streaming structured JSON or SSE to stdout with TUI suppressed.
   - Automatically ingests and hashes workspace `AGENTS.md` with BLAKE3 to maintain constitutional alignment.
   - Routes high-volume subagent tasks to cost-effective models (Gemini 3.5 Flash-Lite).

6. **Docker-Free Bifurcated Sandboxing**:
   - Traditional container runtimes (Docker/OCI) are strictly prohibited.
   - Lightweight workloads: WASI 0.3 WebAssembly components with Cranelift JIT and fuel-metering.
   - Heavyweight workloads: Ephemeral Firecracker microVMs under secondary `jailer` daemons (cgroups v2, seccomp).

7. **Semantic Code Intelligence & Model Routing**:
   - Source code parsing via `tree-sitter` Concrete Syntax Trees (CST) preserving formatting and whitespace.
   - Incremental change detection via BLAKE3 Merkle hashing (`rayon`) into SQLite knowledge graph with BM25 and vector search.
   - Multi-Tier Cognitive Routing: Gemini 3.1 Pro (Tier 3 architectural planning), Gemini 3.8 Flash (default orchestration), Gemini 3.5 Flash-Lite (subagent execution/validation), Deep Research (autonomous collection).

---

## 2. Primary Verification Gates & Qualification Protocol

### A. General Development Gates
1. **AST Blast-Radius Analysis**: Recursive Common Table Expression (CTE) dependency tracing. Refactorings impacting >10 downstream modules escalate to Gemini 3.1 Pro and require human sign-off.
2. **Deterministic Tool Hooks**: All code formatters, linters, and type checkers (`cargo clippy`, `cargo fmt --check`, `ruff`, `mypy`) run via local binaries triggered through `PostToolUse` lifecycle hooks.
3. **Sandbox Attestation Check**: Target sandboxes must hold active jailer policies and valid SPIFFE SVIDs prior to tool execution.

### B. Aura CLI Automated Qualification Gates
Prior to release, `aura-cli` must satisfy four automated verification benchmarks:
1. **Gate 1 (Zero-Copy Memory Benchmark)**: Transmission of a 50MB unified code diff from `aura-core` to `aura-cli` via Zenoh Shared Memory. Validates that process heap memory delta remains flat (Δ <= 512KB for metadata) and render latency is sub-millisecond.
2. **Gate 2 (Attestation Dropping Test)**: An unauthenticated local process attempting to inject messages over Zenoh must be rejected by `spiffe-rustls-tokio` without kernel panics.
3. **Gate 3 (Headless Subagent Throughput)**: Non-interactive execution (`aura-cli --headless -p`) in CI/CD without pseudo-terminal allocation (`-t=false`), streaming valid JSON lines to stdout.
4. **Gate 4 (WASI 0.3 Concurrency Test)**: Concurrent WASI tool invocations must yield futures asynchronously, maintaining a stable 60 FPS TUI frame rate.

---

## 3. Operational Protocols & Guardrails

1. **The Three-Round Rule (Circuit Breaker)**:
   - If an operation, build, or test fails to resolve within **three iterations**, the agent MUST immediately halt its autonomous loop.
   - Formulate a precise diagnostic dossier and ask a targeted clarifying question.

2. **Slash Command Conventions**:
   - `/plan`: Drafts structured architectural specification into `.aura/plans/design-spec.md` with risk assessment and verification gates before modifying code.
   - `/grill-me`: Actively questions the developer regarding edge cases, failure modes, and architectural trade-offs.
   - `/boost`: Multi-agent parallel swarm execution strictly reserved for Tier 3 architectural tasks.
   - `/artifact` (or `Ctrl+R`): Switches `aura-cli` into the Artifact Review Pane for unified diff and blast radius inspection.
   - `/model` (or `Ctrl+M`): Dynamic cognitive model tier selector modal.

3. **Context Hygiene**:
   - Heavy build directories (`target/`, `.venv/`, `node_modules/`, `rootfs/`, `*.sqlite`) are ignored via `.ignore` and `.cursorignore`.

---

## 4. Domain Vocabulary

- **aura-cli**: Pure Rust terminal user interface and IPC client built with Ratatui and Tokio.
- **The Elm Architecture (TEA)**: Model-View-Update unidirectional state architecture driving the TUI.
- **Zenoh SHM**: Eclipse Zenoh 1.5 Shared Memory provider enabling zero-copy typed byte buffer transmission.
- **Cedar Policy Engine**: AWS-developed declarative authorization policy framework regulating tool permissions.
- **AF_VSOCK**: Linux address family for hypervisor-to-guest communications over VirtIO-vsock.
- **SVID**: SPIFFE Verifiable Identity Document used for mutual zero-trust authentication.
- **Blast Radius**: Recursive CTE measurement of code refactoring impact across the repository graph.
