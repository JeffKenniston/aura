---
name: zenoh-shm-benchmarking
description: Benchmarks Zenoh 1.5 Shared Memory allocations, TypedLayout zero-copy deserialization, and IPC throughput between aura-core and aura-cli.
version: 1.0
---

# Zenoh Shared Memory Benchmarking Skill

## Overview
This skill profiles the shared memory data bus linking `aura-core` and `aura-cli`. It ensures large payloads (AST knowledge graphs, unified diffs) are transferred with zero copy overhead.

## Verification Procedures
1. **Profile Memory Allocation**:
   - Verify `z_shm_provider` allocates contiguous memory in RAM.
   - Validate `TypedLayout::<UnifiedDiffPayload>::read_from_zbytes()` casts directly into struct pointers with 0 allocations.

2. **Verify Key Expression Routing**:
   - Test queries on `aura/repository/index?workspace={path}`.
   - Test streaming subscribers on `aura/core/agent/{session_id}/stream`.
   - Test approval publishers on `aura/sessions/{session_id}/approval`.
