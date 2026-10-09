---
name: aura-exe-wasm-domain
description: WASM Runtime constraints. Activate this skill when working on the exe-wasm
  domain.
---
# EXE-WASM Domain Rules
- **Target**: WASI 0.3 for lightweight tools.
- **Async**: Native async streams/futures (completion-based).
- **Engine**: wasmtime with cranelift compilation, pooling instance allocator.
- **Limits**: Enforce fuel budget, memory, and wall-clock limits per invocation.
- **Security**: No ambient authority; only host-granted capabilities.
- **Composition**: Statically composed at deploy time (no hot-loading).
