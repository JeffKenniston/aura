# Aura CLI Architectural Design Specification: Phase 1

## 1. Overview
This architectural plan dictates the Phase 1 implementation for the Aura CLI (`aura-cli`), establishing the core Terminal User Interface (TUI) foundation and the asynchronous execution architecture. The design enforces zero-garbage-collection limits and prioritizes sub-millisecond frame rendering via double buffering.

## 2. Phase Breakdown and Module Boundaries

### Phase 1.1: Binary Target and Baseline Setup
- **Objective:** Establish the foundational cross-compilation matrix.
- **Targets:**
  - `x86_64-unknown-linux-musl` (Statically linked Linux AMD64)
  - `aarch64-unknown-linux-musl` (Statically linked Linux ARM64)
  - `wasm32-wasip2` (WASI 0.3 Component Model)
- **Constraints:** Zero-garbage-collection footprint. The build pipeline (`.cargo/config.toml` and CI) will be strictly bound to these targets to ensure environment portability without `glibc` dependencies.

### Phase 1.2: Terminal Interface Rendering
- **Objective:** Construct the immediate-mode terminal layer.
- **Modules (`src/ui`):**
  - **Double-Buffering Engine:** Integrates `ratatui` (≥ 0.28) and `crossterm` (≥ 0.28). The engine evaluates character cell deltas across frames, mutating only the changed terminal grid spaces.
  - **Tearing Prevention:** The architecture explicitly mitigates screen tearing during rapid LLM token emission by buffering the entire frame internally before flushing via `crossterm` synchronization sequences.

### Phase 1.3: The Asynchronous Event Loop
- **Objective:** Deploy the asynchronous adaptation of The Elm Architecture (TEA).
- **Modules (`src/tea`):**
  - **Work-Stealing Scheduler:** The binary initializes a multi-threaded `tokio` runtime configured for work-stealing to decouple I/O bottlenecks from the rendering thread.
  - **Multiplexer (`tokio::select!`):** The central `loop.rs` multiplexes three distinct streams without thread starvation:
    1. Terminal input streams (`crossterm::event::EventStream`).
    2. Zero-Copy `zenoh-shm` IPC events (`aura-core` telemetry).
    3. Background WASI 0.3 asynchronous channels.

## 3. Substrate Evaluation
- **Execution Substrate:** Pure Rust compiled binary deployed via static musl or WASI 0.3 WebAssembly components.
- **Model Routing Tier:** Gemini 3.1 Pro is allocated for this foundational Tier 3 architectural phase. 

## 4. Invariants Verified
- **Zero FFI:** No dynamic linking or Foreign Function Interfaces (FFI). All components compile strictly to static `musl` or WASI endpoints.
- **Docker-Free Execution:** Target triples explicitly sidestep container runtimes.
- **Decoupled Framing:** The event loop strictly separates model mutation from frame rendering to ensure steady 60 FPS under high token loads.

## 5. Verification Gates
1. **Compilation Matrix Check:** `cargo build --target x86_64-unknown-linux-musl` and `cargo build --target wasm32-wasip2` must successfully link.
2. **Double-Buffering Validation:** High-velocity token streams generated programmatically must be evaluated visually and computationally for cell-delta accuracy.
3. **Starvation Benchmark:** The `tokio` task queue length and executor metrics must prove the terminal input stream remains responsive while WASI or Zenoh channels saturate.

## 6. Rollback Strategy
- Feature modifications are constrained to `feature/cli-foundation`.
- Any breakdown in `wasm32-wasip2` compatibility due to `tokio` or `crossterm` unsupported OS polling will trigger an immediate reversion and substitution with conditional compilation paths.
