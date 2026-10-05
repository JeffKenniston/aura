---
title: Zenoh 1.5 Shared Memory Zero-Copy IPC
description: Defines the zero-copy shared memory architecture, #[repr(C)] memory layouts, and canonical key expressions for aura-cli and aura-core.
tags: [aura-cli, zenoh, zenoh-shm, zero-copy, ipc, memory-alignment]
version: 1.0
---

# Zenoh 1.5 Shared Memory Zero-Copy IPC

## 1. Zero-Copy Shared Memory Data Bus
- All IPC between `aura-cli` and `aura-core` operates over Eclipse Zenoh 1.5 Shared Memory (`zenoh-shm`).
- Payloads exceeding 4KB (AST knowledge graphs, multi-file unified diffs) MUST use `z_shm_provider` memory blocks in RAM.
- Traditional UNIX domain socket and TCP serialization are strictly prohibited for high-volume telemetry.

## 2. `#[repr(C)]` Memory Layout & TypedLayout Mapping
- Payloads in shared memory must use contiguous, C-aligned structures:
  ```rust
  #[repr(C)]
  pub struct UnifiedDiffPayload {
      pub file_count: u32,
      pub total_additions: u32,
      pub total_deletions: u32,
      pub diff_data_len: usize,
      pub data_ptr: *const u8,
  }
  ```
- The receiver maps `zenoh::bytes::ZBytes` directly via `TypedLayout::<T>::read_from_zbytes()` with zero deserialization overhead.

## 3. Semantic Key Expression Namespace
- Context Ingestion: `aura/repository/index?workspace={path}` (Querier to pull pre-computed AST graphs from `aura-core`).
- Execution Telemetry: `aura/core/agent/{session_id}/stream` (Subscriber stream for thought logs, tool invocations, tokens).
- Authorization Pipeline: `aura/sessions/{session_id}/approval` (Publisher channel transmitting human approval decisions).
- Tool Invocation Hooks: `aura/tools/{tool_name}/exec` (Internal sandbox execution queue).
