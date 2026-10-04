# Architecture Design Specification: WASM & Host Tools Integration

## 1. Architecture Discovery
- **Module Ownership**:
  - `aura-core` (Rust Microkernel): Implement the core host functions for `FileSystem`, `AST-Blast-Radius`, and `Ephemeral-Tunnel` within `hypervisor/wasm.rs` or a dedicated `hypervisor/wasm_tools.rs` module. These will be statically linked into the WASM Component Model Linker.
  - `aura-sdk` (Python Substrate): Develop Python handlers mapping to these WASM/host-backed tools, utilizing the Zenoh bus to request execution.
  - `tools/` (Sandboxes): Utilize existing JSON schemas (`filesystem.json`, `ast-blast-radius.json`, `ephemeral-tunnel.json`).
- **Invariants Checked**:
  - **Zero FFI**: Communication remains fully asynchronous over the Zenoh bus.
  - **Docker-Free Sandbox**: Workloads execute within `wasmtime` using WASI 0.3, benefiting from strict fuel-metering and memory pooling allocators rather than containers.
  - **Zero-Trust Identity**: `Ephemeral-Tunnel` mitigates static LLM credential leakage by dynamically minting SPIFFE/SPIRE short-lived identity tokens.

## 2. AST Blast-Radius Pre-Check
- `.agents/knowledge_graph.sqlite` yields **0** upstream callers. The `wasmtime` hypervisor modifications are additive. The blast radius is Low (Zero callers).

## 3. Substrate Selection
- **Execution Environments**: All three tools (FileSystem, AST-Blast-Radius, Ephemeral-Tunnel) represent lightweight, data-centric functional evaluations with low IO overhead. Thus, **WASI 0.3 WebAssembly** running in `wasmtime` is selected.
- **Static Composition Strategy**: As Component Model 1.0 lacks runtime dynamic hot-loading, the tools will be constructed via static composition in the Rust host. Custom WASI host functions will be exposed to the WebAssembly module, allowing WASM-sandboxed agents to safely perform host operations.

## 4. Implementation Specification

### Objective
Develop and integrate WASM and Rust Host-based tools, strictly isolating file modifications, performing pre-refactor dependency tracing via CTEs, and dynamically minting SPIFFE credentials.

### Component 1: Rust Host-Function Implementations (`aura-core`)
- **FileSystem Host Tool**: Expose a `wasi:tools/filesystem` interface. Implement strict path-prefix matching logic (`std::path::Path::starts_with`) restricting all `read`/`write` requests strictly to the assigned workspace directory (`/home/jeff/aura`).
- **AST-Blast-Radius Host Tool**: Expose a `wasi:tools/static-analysis` interface. Use `rusqlite` to execute a recursive Common Table Expression (CTE) query against `.agents/knowledge_graph.sqlite` resolving the complete reverse-dependency chain (`CallChain`). Emits a structured blast-radius score.
- **Ephemeral-Tunnel Host Tool**: Expose a `wasi:tools/credentials` interface. Interacts with the local SPIRE Workload API socket (`/tmp/spire-agent/public/api.sock`) to negotiate an SVID for the active task duration, entirely shielding raw API keys from the LLM prompt.

### Component 2: Static WASM Linker Integration (`aura-core`)
- Update `aura-core/src/hypervisor/wasm.rs`'s `create_linker()` method.
- Statically link the newly developed host tools into the `wasmtime::component::Linker<WasmState>` so that executed `.wasm` components have runtime access to these custom domains.

### Component 3: Python Tool Adapters (`aura-sdk`)
- Create `filesystem.py`, `ast_blast_radius.py`, and `ephemeral_tunnel.py` inside `aura-sdk/aura_sdk/tools/`.
- The AST-Blast-Radius Python tool will evaluate the computed score. If the `callers > 10`, the Python logic will autonomously halt and flag an escalation requirement to Gemini 3.1 Pro.

### Verification Gates
1. **Path-Traversal Block Test**: Assert that a WASM component attempting to access `../../../etc/passwd` or `~/.ssh/id_rsa` fails the FileSystem path-prefix matching.
2. **CTE Logic Test**: Assert that the SQLite recursive CTE correctly returns depth 1..N dependencies for a mocked knowledge graph topology.
3. **WASM Fuel Metering**: Ensure tools do not bypass fuel consumption constraints.
4. **Deterministic Tool Hooks**: Run `cargo clippy`, `cargo fmt --check`, `ruff`, and `mypy`.

### Rollback Plan
- Drop the custom `wasi:tools/*` definitions from the linker.
- Git revert the additions in `aura-core/src/hypervisor/wasm.rs` and `aura-sdk/aura_sdk/tools/`.
