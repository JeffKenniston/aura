---
title: Aura CLI Automated Qualification Gates
description: Defines the four mandatory qualification benchmarks required prior to aura-cli deployment.
tags: [aura-cli, benchmarks, qualification, gates, testing]
version: 1.0
---

# Aura CLI Automated Qualification Gates

Before any build or release of `aura-cli`, the following four automated qualification gates must pass:

1. **Gate 1: Zero-Copy Memory Benchmark**:
   - Transmit a 50MB unified code diff from `aura-core` to `aura-cli` via Zenoh Shared Memory.
   - Profile CLI heap allocation: delta memory allocation must remain flat (Δ <= 512KB for metadata).
   - Render latency must be sub-millisecond.

2. **Gate 2: Attestation Dropping Test**:
   - Inject an unauthenticated local connection attempting tool approvals over Zenoh without a SPIFFE SVID.
   - Verify that `spiffe-rustls-tokio` terminates the connection immediately without panic or crash.

3. **Gate 3: Headless Subagent Throughput**:
   - Execute `aura-cli --headless -p "..."` inside a non-interactive environment with pseudo-terminal disabled (`-t=false`).
   - Validate that clean, valid JSON lines or SSE events stream to stdout.

4. **Gate 4: WASI 0.3 Concurrency Test**:
   - Execute concurrent WASI tool calls yielding async futures under `wasmtime`.
   - Confirm the TUI frame rate maintains a constant 60 FPS without stutter or dropped frames.
