# Dispatch Assignment — worker_m1_1

## Identity
- Role: Implementation Worker (teamwork_preview_worker)
- Working Directory: /home/jeff/aura/.agents/teamwork/worker_m1_1/
- Parent Orchestrator: orchestrator_5 (Conversation ID: 24a6c483-94de-4894-8f07-01be6a2b194e)

## Mandatory Input Documents
You MUST read the following documents before writing any code:
- `/home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md` (Read verbatim — do NOT skip)
- `/home/jeff/aura/.agents/teamwork/orchestrator_5/PROJECT.md`
- `/home/jeff/aura/.agents/teamwork/explorer_m1_1/handoff.md` (Blueprint for M0-SPIKE-03 & HOST-002: runtime builder, AuraConfig, metrics)
- `/home/jeff/aura/.agents/teamwork/explorer_m1_2/handoff.md` (Blueprint for HOST-002: blocking pool isolation, offload primitives, starvation tests)
- `/home/jeff/aura/.agents/teamwork/explorer_m1_3/handoff.md` (Blueprint for HOST-004: transport config, http3 helpers, 5k concurrency test)

## Mandatory Integrity Warning
> DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

## Exclusive Write Ownership
You own the following files exclusively:
- `/home/jeff/aura/.cargo/config.toml`
- `/home/jeff/aura/aura-core/src/config/mod.rs`
- `/home/jeff/aura/aura-core/src/runtime/mod.rs`
- `/home/jeff/aura/aura-core/src/lib.rs`
- `/home/jeff/aura/aura-cli/src/main.rs`
- `/home/jeff/aura/aura-core/src/transport/http3.rs`
- `/home/jeff/aura/aura-core/tests/http3_stream_concurrency_test.rs`

## Implementation Steps
1. **Enable tokio_unstable in Workspace**:
   Create `/home/jeff/aura/.cargo/config.toml` with:
   ```toml
   [build]
   rustflags = ["--cfg", "tokio_unstable"]
   ```
2. **Update `aura-core/src/config/mod.rs`**:
   - Add `RuntimeConfig` (`worker_threads`, `max_blocking_threads`, `thread_keep_alive_secs`) with defaults and validation.
   - Add `pub runtime: RuntimeConfig` to `AuraConfig` and update `AuraConfig::validate()`.
   - Update `TransportConfig::default()`: set `initial_max_streams_bidi: 10_000` and `initial_max_data: 50_000_000`.
   - Add unit tests for `RuntimeConfig` defaults, validation, and TOML deserialization.
3. **Implement `aura-core/src/runtime/mod.rs`**:
   - Implement `detect_hardware_parallelism()`, `init_tokio_pool()`, `init_tokio_pool_with_config()`.
   - Implement thread naming `"aura-core-worker"`, max blocking threads 512, thread keep alive 10s.
   - Implement `spawn_blocking_compute` and `run_blocking_compute`.
   - Implement `RuntimeHealthSnapshot`, `WorkerHealthSnapshot`, and `health_snapshot()` using `tokio::runtime::RuntimeMetrics`.
   - Implement all unit tests from `explorer_m1_1` and `explorer_m1_2` (hardware parallelism, default/custom workers, worker naming, thread distribution, health snapshot, timer preservation under blocking load, channel dispatch latency, fair work distribution, result propagation).
4. **Update `aura-core/src/lib.rs`**:
   - In `init()` and `init_with_config()`, verify and register the running Tokio runtime handle.
   - Add synchronous entry points `run()` and `run_with_config()` that build the pool and call `rt.block_on(init())`.
5. **Update `aura-cli/src/main.rs`**:
   - Bootstrap the runtime pool via `aura_core::runtime::init_tokio_pool(None)?`.
6. **Update `aura-core/src/transport/http3.rs`**:
   - Add `build_server_quiche_config` and `build_client_quiche_config` helper methods.
7. **Create `aura-core/tests/http3_stream_concurrency_test.rs`**:
   - Copy or implement the complete empirical 5,000 stream load test from `explorer_m1_3`'s artifact at `/home/jeff/aura/.agents/teamwork/explorer_m1_3/proposed_http3_stream_concurrency_test.rs`.
8. **Verification**:
   - Run `cargo check --all-targets`
   - Run `cargo test -p aura-core --lib`
   - Run `cargo test -p aura-core --test http3_stream_concurrency_test -- --nocapture`
   - Run `cargo run -p aura-cli`
   - Ensure 100% of tests pass cleanly with zero compiler warnings or errors.

## Output Requirements
Write your detailed handoff report at:
`/home/jeff/aura/.agents/teamwork/worker_m1_1/handoff.md`
Include:
- Exact files modified/created
- Verification commands executed and exact stdout/stderr
- Test results summary
When done, message orchestrator_5 (24a6c483-94de-4894-8f07-01be6a2b194e).
