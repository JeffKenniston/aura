# Dispatch Assignment — explorer_m1_2

## Identity
- Role: Codebase Explorer (teamwork_preview_explorer)
- Working Directory: /home/jeff/aura/.agents/teamwork/explorer_m1_2/
- Parent Orchestrator: orchestrator_5 (Conversation ID: 24a6c483-94de-4894-8f07-01be6a2b194e)

## Mandatory Input Documents
You MUST read the following documents before starting your investigation:
- `/home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md` (Read verbatim — do NOT skip)
- `/home/jeff/aura/M0_M3_Audit_Report.md` (Specifically Section 4.2 for HOST-002)
- `/home/jeff/aura/requirements.md`
- `/home/jeff/aura/.agents/teamwork/orchestrator_5/PROJECT.md`

## Objective & Scope
Investigate and design the blocking workload isolation and non-blocking concurrency architecture for **HOST-002** in `aura-core`:
1. Blocking vs Asynchronous Task Separation
   - Analyze how CPU-bound or blocking operations in `aura-core` (such as Wasmtime JIT compilation in `hypervisor/wasm.rs`, synchronous file I/O in Cedar/policy enforcer, or Firecracker Unix socket calls) interact with the Tokio runtime.
   - Design helper primitives or pool wrappers (`spawn_blocking_compute`, dedicated blocking thread pool sizing) ensuring that worker threads are never blocked on heavy computations or syscalls.
2. Starvation & Scheduling Invariants
   - Design unit and integration tests in `aura-core/src/runtime/mod.rs` proving that intensive blocking operations offloaded to the blocking pool do not stall async timer ticks, channel communication, or Zenoh event processing.
   - Design tests verifying fair work distribution across worker threads.

## Scope Boundaries
- You are read-only. Do NOT modify source code files.
- Recommend exact structs, functions, signatures, and file diffs for the Worker.
- Write your complete findings to `/home/jeff/aura/.agents/teamwork/explorer_m1_2/handoff.md`.

## Output Requirements
Write a comprehensive handoff report at:
`/home/jeff/aura/.agents/teamwork/explorer_m1_2/handoff.md`
Format:
- 1. Observation (existing blocking points, task distribution)
- 2. Logic Chain (thread pool sizing, offloading strategy)
- 3. Concrete Implementation Blueprint (exact code snippets, helper functions, wrappers)
- 4. Test Design (starvation detection test, timer preservation test)
- 5. Verification Commands

Send a concise completion message back to the orchestrator once complete.


## 2026-10-10T05:46:24Z
[Message] sender=24a6c483-94de-4894-8f07-01be6a2b194e priority=MESSAGE_PRIORITY_HIGH
You are explorer_m1_2, a teamwork_preview_explorer subagent.
Your assigned working directory is: /home/jeff/aura/.agents/teamwork/explorer_m1_2/
Your caller orchestrator is: 24a6c483-94de-4894-8f07-01be6a2b194e

MANDATORY FIRST STEP:
Read /home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md before performing any work.
Then read your detailed dispatch assignment at:
/home/jeff/aura/.agents/teamwork/explorer_m1_2/DISPATCH.md

Objective:
Investigate HOST-002 in aura-core:
- Blocking vs asynchronous task separation: offloading heavy compute (e.g. Wasmtime JIT compilation, crypto, disk I/O) to blocking pool (spawn_blocking)
- Thread starvation prevention for latency-sensitive tasks (Zenoh dispatch, QUIC listener)
- Verification test design in aura-core/src/runtime/mod.rs proving timer preservation and non-blocking scheduling under heavy compute load

Write your full handoff report to:
/home/jeff/aura/.agents/teamwork/explorer_m1_2/handoff.md

When done, send a message to your caller (24a6c483-94de-4894-8f07-01be6a2b194e) with a summary of findings and the path to your handoff.md.
