# Aura CLI Architectural Design Specification

## 1. Overview
This document outlines the architectural plan for the Aura CLI (`aura-cli`), the pure Rust presentation node and IPC client. It covers the Terminal Rendering Engine, Progressive Authorization integration, Headless Subagent Pipelines, and the requisite Engineering Validation Gates.

## 2. Target Components and Module Boundaries
- **`aura-cli` (Execution Host - Rust):**
  - **Main Event Loop (`src/tea/loop.rs`):** A central `tokio::select!` block multiplexing user keyboard signals, Zero-Copy Zenoh IPC messages, and native WASI 0.3 async telemetry channels.
  - **State Model & Update (`src/tea/model.rs`, `src/tea/update.rs`):** The immutable `AppModel` and pure update functions for the asynchronous TEA architecture.
  - **View & Graphics (`src/ui/view.rs`, `src/ui/graphics.rs`):** Immediate-mode rendering logic outputting to `ratatui` (>= 0.28). Integration with `ratatui-image` for inline Sixel/Kitty Graphics.
  - **Progressive Authorization (`src/auth/cedar.rs`):** Integration with the Cedar Policy Engine (`cedar-policy`, `cedarling`) to evaluate proposed tool executions against four tiers: `strict`, `request-review`, `proceed-in-sandbox`, and `always-proceed`.
  - **Artifact Review Pane (`src/ui/components/artifact_review.rs`):** Interactive UI component that intercepts mutations requiring explicit human approval, displaying zero-copy unified diffs to the user.
  - **Headless Mode (`src/headless/pipeline.rs`):** The `--headless -p` execution mode. Bypasses terminal UI, hashes the `AGENTS.md` context using BLAKE3, routes execution to Gemini 3.5 Flash-Lite, and emits structured JSON/SSE to POSIX stdout.
  - **Zenoh Client (`src/ipc/zenoh.rs`):** Handles `zenoh-shm` with `#[repr(C)]` layouts and `TypedLayout` zero-deserialization mapping.

## 3. Substrate Evaluation
- **Execution Substrate:** Pure Rust compiled binary (`aura-cli`). No Docker sandboxing is used. WASI 0.3 WebAssembly handles lightweight tasks, and Firecracker microVMs are utilized for OS integration tasks.
- **Model Routing Tier:**
  - **Tier 3 (Architecture Planning):** Gemini 3.1 Pro (Completed by this plan).
  - **Feature Implementation:** Gemini 3.8 Flash.
  - **Subagent Delegation & Headless Pipelines:** Gemini 3.5 Flash-Lite.

## 4. Invariants Verified
- **Zero FFI:** The presentation layer is pure Rust. It communicates with Python agents or other processes exclusively via the asynchronous Zenoh bus. No `PyO3` or C-FFI will be introduced.
- **Stateless Presentation:** The CLI contains no LLM network stacks or database drivers. State is derived entirely from the IPC event stream and user inputs.
- **Asynchronous TEA Concurrency:** Rendering is fully decoupled from I/O. The `tokio::select!` loop guarantees that IPC message deserialization or WASI telemetry streams will not starve UI frame rendering (targeting 60 FPS).
- **Transport Attestation (Layer 1 Security):** The `aura-cli` Zenoh client enforces mutual TLS via SPIFFE SVIDs (`spiffe-rustls-tokio`) targeting `spiffe://aura.local/workload/aura-cli`.

## 5. Step-by-step Verification Gates
To qualify the `aura-cli` engine prior to release, the following automated benchmarks will be executed:
- **Gate 1 (Zero-Copy Memory Benchmark):** Transmit a 50MB structured unified code diff from `aura-core` over `zenoh-shm`. Validate that `aura-cli` heap memory delta is ≤ 512KB and UI render latency remains sub-millisecond.
- **Gate 2 (Attestation Test):** Launch an unauthenticated local process attempting to inject messages to the `aura-cli` IPC port. Verify rejection via `spiffe-rustls-tokio` without kernel panics.
- **Gate 3 (Headless Throughput):** Run `aura-cli --headless -p "test"` in CI without PTY allocation (`-t=false`). Verify valid structured JSON lines or SSE are emitted to stdout and TUI rendering is entirely bypassed.
- **Gate 4 (WASI 0.3 Concurrency):** Simulate concurrent asynchronous future yields from WASI tool invocations. Validate that the TEA event loop maintains a stable 60 FPS without starvation.

## 6. Rollback Strategy
- **Version Control Reversion:** If any of the above verification gates fail during integration, the feature branch will be reverted (`git revert <commit>`).
- **Circuit Breaker Integration:** If a component repeatedly fails to compile or pass tests within three iterations during autonomous development, the agent will halt and formulate a clarifying diagnostic dossier for the human reviewer.
