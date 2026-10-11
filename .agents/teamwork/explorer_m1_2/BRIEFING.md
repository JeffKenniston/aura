# BRIEFING — 2026-10-10T05:54:00Z

## Mission
Investigate HOST-002 in aura-core: blocking workload isolation, asynchronous task separation, thread starvation prevention, and verification test design.

## 🔒 My Identity
- Archetype: teamwork_preview_explorer
- Roles: [explorer, investigator, designer]
- Working directory: /home/jeff/aura/.agents/teamwork/explorer_m1_2
- Original parent: 24a6c483-94de-4894-8f07-01be6a2b194e
- Milestone: M1 (HOST-002 Blocking vs Async Task Separation)

## 🔒 Key Constraints
- Read-only investigation — do NOT implement / modify source code directly
- Strict evidence chain (file paths, line numbers, exact code)
- Produce handoff.md following 5-component format
- Deliver blueprints, code diffs, helper primitives, and test specifications for workers

## Current Parent
- Conversation ID: 24a6c483-94de-4894-8f07-01be6a2b194e
- Updated: 2026-10-10T05:54:00Z

## Investigation State
- **Explored paths**:
  - `aura-core/src/runtime/mod.rs` (facade stub, lines 3-6)
  - `aura-core/src/hypervisor/wasm.rs` (Cranelift JIT compilation on worker threads, line 99)
  - `aura-core/src/hypervisor/firecracker.rs` (synchronous UnixStream socket requests in `send_api_request` and `start_microvm`, lines 510, 545)
  - `aura-core/src/bus/mod.rs` (Zenoh task dispatch loop, lines 136-164)
  - `aura-core/src/knowledge/indexer.rs` (Rayon BLAKE3 file hashing, lines 7-9, 20-23)
  - `aura-core/src/identity/mod.rs` (synchronous Cedar policy parsing and evaluation, lines 438, 480)
- **Key findings**:
  - Tokio multi-threaded runtime requires explicit separation of asynchronous worker threads from blocking threads (`max_blocking_threads = 512`, `thread_keep_alive = 10s`).
  - Without offloading, unyielded CPU bursts in JIT or crypto completely saturate worker threads, starving QUIC ACK timers and Zenoh dispatch loops.
  - Designed `spawn_blocking_compute` and `run_blocking_compute` in `aura-core/src/runtime/mod.rs`.
  - Designed complete 5-part verification test suite proving timer preservation (15ms interval maintained during 150ms compute bursts), sub-25ms channel latency, and fair multi-worker load distribution.
- **Unexplored areas**: none (HOST-002 fully scoped, analyzed, and blueprint created).

## Key Decisions Made
- Offload Cranelift JIT compilation (`compile_component_blocking`) and microVM boot (`start_microvm`) to the blocking pool.
- Spawn detached async tasks in Zenoh `start_task_listener` to prevent message queue head-of-line blocking.
- Sized Tokio blocking pool to 512 threads with 10s idle keep-alive.
- Authored comprehensive handoff report in `handoff.md`.

## Artifact Index
- `/home/jeff/aura/.agents/teamwork/explorer_m1_2/DISPATCH.md` — Dispatch instructions
- `/home/jeff/aura/.agents/teamwork/explorer_m1_2/BRIEFING.md` — Working memory
- `/home/jeff/aura/.agents/teamwork/explorer_m1_2/progress.md` — Liveness heartbeat
- `/home/jeff/aura/.agents/teamwork/explorer_m1_2/handoff.md` — Final investigation handoff report
