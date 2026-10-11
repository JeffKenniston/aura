# Dispatch for Worker M1 Verify

You are `worker_m1_verify` (role: `teamwork_preview_worker`).
Your assigned working directory is `/home/jeff/aura/.agents/teamwork/worker_m1_verify/`.

## Mandatory Reading:
- User Request: `/home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md`
- Scope: `/home/jeff/aura/.agents/teamwork/orchestrator_7/PROJECT.md`
- Predecessor Worker Progress: `/home/jeff/aura/.agents/teamwork/worker_m1_1/progress.md`
- Domain Skill: `/home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-host-domain/SKILL.md`

## Milestone 1 Implementation Authored:
1. `.cargo/config.toml` (rustflags enabling `tokio_unstable`)
2. `aura-core/src/config/mod.rs` (RuntimeConfig, defaults 10,000 max streams, 50MB window)
3. `aura-core/src/runtime/mod.rs` (Runtime pool builder, metrics via `tokio_unstable`, blocking compute isolation)
4. `aura-core/src/lib.rs` (Runtime registration, run_with_config)
5. `aura-cli/src/main.rs` (CLI bootstrap with runtime pool)
6. `aura-core/src/transport/http3.rs` (QUIC config with 10k streams and large receive window)
7. `aura-core/tests/http3_stream_concurrency_test.rs` (empirical QUIC 5k stream concurrency test)

## Mandatory Tasks:
1. Run `cargo test -p aura-core` to verify all 103 unit tests pass.
2. Run `cargo test -p aura-core --test http3_stream_concurrency_test -- --nocapture` to verify the 5,000 stream concurrency test passes.
3. Check `cargo check --all-targets` to verify clean compilation with zero warnings/errors.
4. If any tests fail, inspect and fix any issues in the M1 files without using any mocks or stubs.
5. Write your complete verification results and test outputs in `/home/jeff/aura/.agents/teamwork/worker_m1_verify/handoff.md`.

## Mandatory Integrity Warning:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.


## 2026-10-11T00:51:43Z
You are worker_m1_verify.
Your working directory is `/home/jeff/aura/.agents/teamwork/worker_m1_verify/`.
Read your task assignment in `/home/jeff/aura/.agents/teamwork/worker_m1_verify/DISPATCH.md`.

Mandatory Reading:
- User Request: `/home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md`
- Scope: `/home/jeff/aura/.agents/teamwork/orchestrator_7/PROJECT.md`
- Predecessor Worker Progress: `/home/jeff/aura/.agents/teamwork/worker_m1_1/progress.md`
- Domain Skill: `/home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-host-domain/SKILL.md`

Your tasks:
1. Verify `cargo test -p aura-core` (verify all unit tests pass).
2. Verify `cargo test -p aura-core --test http3_stream_concurrency_test -- --nocapture` (verify 5,000 stream concurrency test passes).
3. Verify `cargo check --all-targets`.
4. Fix any failures if encountered (NO mocks/stubs per AGENTS.md).
5. Document all commands and outputs in `/home/jeff/aura/.agents/teamwork/worker_m1_verify/handoff.md`.
6. Send a message to orchestrator upon completion.

MANDATORY INTEGRITY WARNING:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.
