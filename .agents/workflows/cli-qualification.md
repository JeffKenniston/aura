---
command: /cli-qualify
description: Runs the automated 4-gate qualification benchmarks for aura-cli.
version: 1.0
---

# /cli-qualify Workflow: Aura CLI Qualification Benchmarks

Execute the four automated gates:
1. **Run Zero-Copy Memory Benchmark**:
   - `cargo bench --bench shm_zero_copy --package aura-cli`
   - Validate heap allocation delta <= 512KB.
2. **Run Attestation Dropping Test**:
   - `cargo test --package aura-cli test_unauthenticated_connection_dropped`
3. **Run Headless Subagent Test**:
   - `cargo run --package aura-cli -- --headless -p "Validate syntax" --dir .`
   - Verify stdout contains valid JSON records only.
4. **Run WASI 0.3 Concurrency Test**:
   - `cargo test --package aura-cli test_wasi_async_framerate_stability`
