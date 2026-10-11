# Dispatch for Worker M1_2

You are `worker_m1_2` (role: `teamwork_preview_worker`).
Your working directory is: `/home/jeff/aura/.agents/teamwork/worker_m1_2/`

## Objective:
Verify Milestone 1: Runtime & Concurrency (M0-SPIKE-03, HOST-002, HOST-004).

## Context:
`worker_m1_1` has authored the following files for Milestone 1:
1. `.cargo/config.toml` (rustflags enabling `tokio_unstable`)
2. `aura-core/src/config/mod.rs` (RuntimeConfig, defaults 10,000 max streams, 50MB window)
3. `aura-core/src/runtime/mod.rs` (Runtime pool builder, metrics via `tokio_unstable`, blocking compute isolation)
4. `aura-core/src/lib.rs` (Runtime registration, run_with_config)
5. `aura-cli/src/main.rs` (CLI bootstrap with runtime pool)
6. `aura-core/src/transport/http3.rs` (QUIC config with 10k streams and large receive window)
7. `aura-core/tests/http3_stream_concurrency_test.rs` (empirical QUIC 5k stream concurrency test)

## Mandatory Tasks:
1. Run `cargo test -p aura-core` to verify all 103 unit tests pass.
2. Run `cargo test -p aura-core --test http3_stream_concurrency_test -- --nocapture` to verify 5,000 stream concurrency test passes.
3. Check `cargo check --all-targets` and verify no warnings or errors.
4. If any tests fail, inspect and fix the implementation in the M1 files without using any mocks or stubs.
5. Write your complete verification results and test outputs in `/home/jeff/aura/.agents/teamwork/worker_m1_2/handoff.md`.

## Mandatory Integrity Warning:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.
