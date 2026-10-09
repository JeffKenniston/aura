---
name: aura-host-domain
description: Execution Host constraints and architecture. Activate this skill when
  working on the host domain.
---
# HOST Domain Rules (Execution Host)
- **Language**: Rust
- **Runtime**: tokio async runtime (no blocking syscalls on worker threads).
- **Embedded WASM**: Embed wasmtime in-process.
- **Scale**: Multiplex 5,000+ concurrent HTTP/3 streams without thread starvation.
- **Portability**: Native on Linux x86_64/aarch64 and Windows via WSL2.
- **FFI**: No FFI bindings to Python or Node.js.
- **Resilience**: Supervise child processes and restart with exponential backoff.
- **Lifecycle**: Graceful shutdown, draining in-flight streams. Support administrative `drain` command.
