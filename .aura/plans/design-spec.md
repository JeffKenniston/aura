# Architecture Design Specification: Universal Tree-Sitter Language Support

## 1. Architecture Discovery
- **Module Ownership**: 
  - `aura-core` (Rust Microkernel): Expansion of the `knowledge::indexer` to support a universal language matrix.
- **Invariants Checked**: 
  - **Zero FFI**: No Rust-to-Python FFI. All AST parsing and metadata extraction runs securely within the Rust host and is stored in SQLite.
  - **Docker-Free Sandbox**: Universal language support will avoid bloated Docker containers by utilizing native bindings and lightweight WASM modules.
  - **SPIFFE Identity**: Unchanged; filesystem and registry access will be metered and authenticated via SVID.

## 2. AST Blast-Radius Pre-Check
- `.agents/knowledge_graph.sqlite` currently holds 0 live references for the `extract_cst` function outside of internal tests. The blast radius for extending the language routing matrix is zero.

## 3. Substrate Selection
- **Rust Host (Native C-FFI)**: Core ecosystem languages (Rust, Python, TypeScript, JavaScript, Go) will be statically linked via their respective `tree-sitter-*` crates to guarantee maximum throughput and zero-latency parsing for primary toolchains.
- **WASI 0.3 WebAssembly (`wasmtime`)**: To support "all modern languages in all domains" without incurring extreme binary bloat and hours of C++ compilation times, long-tail languages (e.g., Haskell, Elixir, Erlang, Swift, Kotlin, Perl) will be supported via dynamically loaded `.wasm` Tree-Sitter grammars executed securely in the existing `wasmtime` sandbox.
- **Cognitive Tier Allocation**:
  - **Gemini 3.1 Pro**: Designing the dual-routing (Native + WASM) parser interface.

## 4. Implementation Specification

### Objective
Scale the semantic intelligence engine to universally support all modern programming languages by introducing a hybrid parsing architecture: Native C-FFI for high-frequency languages, and dynamic WebAssembly (WASM) loading for infinite extensibility across all domains.

### Component 1: Native "Core Pack" Integration
- Update `Cargo.toml` in `aura-core` to include the standard high-frequency parsers:
  - `tree-sitter-rust`, `tree-sitter-python`, `tree-sitter-javascript`, `tree-sitter-typescript`, `tree-sitter-go`, `tree-sitter-c`, `tree-sitter-cpp`, `tree-sitter-java`, `tree-sitter-c-sharp`, `tree-sitter-ruby`.
- Implement a static `ParserRegistry` that instantiates these native languages instantly.

### Component 2: Dynamic WASM Grammar Loader
- Introduce a WASM grammar registry mapping extensions (e.g., `.ex`, `.hs`) to local `grammar.wasm` binaries.
- Utilize the `wasmtime` crate (already present in the workspace) to dynamically load and execute WASM-compiled Tree-Sitter grammars.
- Implement fuel-metering on the WASM execution to prevent malicious or malformed grammars from causing infinite loops in the microkernel.

### Component 3: Universal Query Abstraction
- Since AST node names differ across languages (e.g., `function_definition` in Python vs `function_item` in Rust), implement a query abstraction layer.
- Create a `.aura/queries/` directory containing language-specific `tags.scm` (Scheme files) adhering to the standard Tree-Sitter tags specification.
- The Rust engine will load the appropriate `tags.scm` for the detected language to extract `definition.function`, `definition.class`, and `reference.call` captures universally, piping them into the Three-Pass SQLite resolution mechanism.

### Verification Gates
1. **Build Performance**: Ensure `cargo check` remains performant by only statically linking the top 10 languages.
2. **Universal Tags Extraction**: Write a test fixture containing the top 10 statically linked languages (Rust, Python, JavaScript, TypeScript, Go, C, C++, Java, C#, Ruby) and assert that `KnowledgeGraph::insert_local_definition` accurately records their primary class/function symbols.
3. **WASM Sandboxing**: Attempt to load a malformed WASM grammar and verify `wasmtime` fuel-metering successfully traps and terminates the execution without crashing `aura-core`.

### Rollback Plan
- Revert `Cargo.toml` native dependencies.
- Remove the `wasmtime` integration from the indexer.
- Clear the `.aura/queries/` directory.
