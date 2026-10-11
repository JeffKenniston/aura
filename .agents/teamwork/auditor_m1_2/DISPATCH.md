# Dispatch for Forensic Auditor M1 (Replacement)

You are `auditor_m1_2` (role: Forensic Auditor, type: `teamwork_preview_auditor`).
Your assigned working directory is `/home/jeff/aura/.agents/teamwork/auditor_m1_2/`.

## Mandatory Reading:
- User Request: `/home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md`
- Scope: `/home/jeff/aura/.agents/teamwork/orchestrator_7/PROJECT.md`
- Repository Constitution: `/home/jeff/aura/AGENTS.md` (Strict zero mock/stub policy)
- Predecessor Context: `/home/jeff/aura/.agents/teamwork/worker_m1_1/progress.md`

## Milestone 1 Files to Inspect & Audit:
1. `.cargo/config.toml` (rustflags enabling `tokio_unstable`)
2. `aura-core/src/config/mod.rs` (RuntimeConfig, defaults 10,000 max streams, 50MB window)
3. `aura-core/src/runtime/mod.rs` (Runtime pool builder, metrics via `tokio_unstable`, blocking compute isolation)
4. `aura-core/src/lib.rs` (Runtime registration, run_with_config)
5. `aura-cli/src/main.rs` (CLI bootstrap with runtime pool)
6. `aura-core/src/transport/http3.rs` (QUIC config with 10k streams and large receive window)
7. `aura-core/tests/http3_stream_concurrency_test.rs` (empirical QUIC 5k stream concurrency test)

## Mandatory Tasks:
1. Run all forensic integrity checks on the M1 implementation files.
2. Verify that there are ABSOLUTELY NO mocks, stubs, TODOs, or hardcoded fake responses in production paths (AGENTS.md).
3. Verify that `tokio::runtime::Builder` is genuinely multi-threaded and `RuntimeMetrics` accesses genuine Tokio runtime metrics.
4. Verify that `http3_stream_concurrency_test.rs` genuinely exercises 5,000 concurrent QUIC streams against the local endpoint and does not simulate or fake stream execution.
5. Execute `cargo test -p aura-core --lib` and `cargo test -p aura-core --test http3_stream_concurrency_test` to verify passing test execution.
6. Write your comprehensive audit report in `/home/jeff/aura/.agents/teamwork/auditor_m1_2/handoff.md`.
7. Conclude with a clear binary verdict: **CLEAN** or **INTEGRITY VIOLATION**.
8. Send a message to orchestrator with your verdict and findings.

## 2026-10-11T03:20:55Z
You are auditor_m1_2 (role: Forensic Auditor, type: teamwork_preview_auditor).
Your working directory is `/home/jeff/aura/.agents/teamwork/auditor_m1_2/`.
Read your task assignment in `/home/jeff/aura/.agents/teamwork/auditor_m1_2/DISPATCH.md`.

Mandatory Reading:
- User Request: `/home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md`
- Scope: `/home/jeff/aura/.agents/teamwork/orchestrator_7/PROJECT.md`
- Repository Constitution: `/home/jeff/aura/AGENTS.md` (Strict zero mock/stub policy)
- Predecessor Context: `/home/jeff/aura/.agents/teamwork/worker_m1_1/progress.md`

Your tasks:
1. Inspect the M1 files:
   - `.cargo/config.toml`
   - `aura-core/src/config/mod.rs`
   - `aura-core/src/runtime/mod.rs`
   - `aura-core/src/lib.rs`
   - `aura-cli/src/main.rs`
   - `aura-core/src/transport/http3.rs`
   - `aura-core/tests/http3_stream_concurrency_test.rs`
2. Run integrity forensics: verify zero mocks, zero stubs, zero TODOs in production paths. Verify genuine multi-threaded Tokio runtime pool builder, authentic runtime metrics via tokio_unstable, and authentic 5,000 QUIC stream load test.
3. Execute `cargo test -p aura-core --lib` and `cargo test -p aura-core --test http3_stream_concurrency_test`.
4. Write your full report in `/home/jeff/aura/.agents/teamwork/auditor_m1_2/handoff.md`.
5. Conclude with a clear binary verdict: CLEAN or INTEGRITY VIOLATION.
6. Send a message to orchestrator with your verdict.
