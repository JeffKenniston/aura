---
name: cli-qualification-gates
description: Executes and validates the four automated qualification benchmarks for aura-cli prior to release.
version: 1.0
---

# Aura CLI Qualification Gates Skill

## Overview
This skill operationalizes the four qualification benchmarks mandated by the `aura-cli design spec`. It tests memory stability, security boundaries, headless automation, and async frame rates.

## Verification Procedures
1. **Gate 1: Zero-Copy Memory Benchmark**:
   - Transmit 50MB unified code diff via Zenoh Shared Memory (`zenoh-shm`).
   - Monitor heap allocation: verify that process heap delta remains <= 512KB for metadata.
   - Verify render latency remains sub-millisecond.

2. **Gate 2: Attestation Dropping Test**:
   - Attempt an unauthenticated IPC connection over Zenoh simulating an untrusted local process.
   - Verify `spiffe-rustls-tokio` rejects the connection and drops the stream without triggering host kernel panics.

3. **Gate 3: Headless Subagent Throughput**:
   - Execute `aura-cli --headless -p "..."` in non-interactive environment with `-t=false`.
   - Validate that clean JSON stream / SSE outputs to stdout without TUI escape codes.

4. **Gate 4: WASI 0.3 Concurrency Test**:
   - Trigger concurrent WASI component tool calls while rendering TUI frames.
   - Validate that TUI frame rate maintains a steady 60 FPS without stutter or frame drops.
