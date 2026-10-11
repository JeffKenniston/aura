# Dispatch Assignment — explorer_m1_3

## Identity
- Role: Codebase Explorer (teamwork_preview_explorer)
- Working Directory: /home/jeff/aura/.agents/teamwork/explorer_m1_3/
- Parent Orchestrator: orchestrator_5 (Conversation ID: 24a6c483-94de-4894-8f07-01be6a2b194e)

## Mandatory Input Documents
You MUST read the following documents before starting your investigation:
- `/home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md` (Read verbatim — do NOT skip)
- `/home/jeff/aura/M0_M3_Audit_Report.md` (Specifically Section 4.3 for HOST-004)
- `/home/jeff/aura/requirements.md`
- `/home/jeff/aura/.agents/teamwork/orchestrator_5/PROJECT.md`

## Objective & Scope
Investigate and design the empirical load test for **HOST-004** (High-Concurrency Stream Multiplexing Scale asserting 5,000+ concurrent streams) in `aura-core`:
1. Quiche & HTTP/3 Transport Architecture
   - Inspect `/home/jeff/aura/aura-core/src/transport/http3.rs` and `/home/jeff/aura/aura-core/src/config/mod.rs`.
   - Verify `initial_max_streams_bidi` setting (currently 10,000).
   - Analyze how streams are accepted, processed, and multiplexed over UDP/QUIC.
2. 5,000+ Concurrent Streams Empirical Stress Test
   - Design a dedicated empirical test in `/home/jeff/aura/aura-core/tests/http3_stream_concurrency_test.rs`.
   - Must use `tokio::spawn` to drive 5,000 concurrent client streams (or batches of concurrent streams) against a local HTTP/3 listener.
   - Assert all 5,000 streams succeed without packet loss, stream resets, or worker starvation.
   - Design metric collection: total completion time, P99 round-trip latency, and peak memory usage.
   - Avoid ephemeral OS port exhaustion or resource exhaustion on Linux.

## Scope Boundaries
- You are read-only. Do NOT modify source code files.
- Recommend exact test code, helper functions, and runner commands for the Worker.
- Write your complete findings to `/home/jeff/aura/.agents/teamwork/explorer_m1_3/handoff.md`.

## Output Requirements
Write a comprehensive handoff report at:
`/home/jeff/aura/.agents/teamwork/explorer_m1_3/handoff.md`
Format:
- 1. Observation (current HTTP/3 transport capabilities, configuration, tests)
- 2. Logic Chain (concurrency generation strategy, client-server simulation model)
- 3. Concrete Implementation Blueprint (complete test file code for `http3_stream_concurrency_test.rs`)
- 4. Empirical Performance & Scale Invariants
- 5. Verification Commands

Send a concise completion message back to the orchestrator once complete.


## 2026-10-10T05:46:24Z
From: 24a6c483-94de-4894-8f07-01be6a2b194e
Content:
You are explorer_m1_3, a teamwork_preview_explorer subagent.
Your assigned working directory is: /home/jeff/aura/.agents/teamwork/explorer_m1_3/
Your caller orchestrator is: 24a6c483-94de-4894-8f07-01be6a2b194e

MANDATORY FIRST STEP:
Read /home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md before performing any work.
Then read your detailed dispatch assignment at:
/home/jeff/aura/.agents/teamwork/explorer_m1_3/DISPATCH.md

Objective:
Investigate HOST-004 in aura-core:
- High-concurrency QUIC stream multiplexing scale (5,000+ streams)
- Transport config in aura-core/src/config/mod.rs and aura-core/src/transport/http3.rs
- Empirical load test design in aura-core/tests/http3_stream_concurrency_test.rs asserting 5,000+ concurrent streams without packet loss, stream resets, or worker starvation
- Metrics collection: throughput, P99 round-trip latency, memory usage

Write your full handoff report to:
/home/jeff/aura/.agents/teamwork/explorer_m1_3/handoff.md

When done, send a message to your caller (24a6c483-94de-4894-8f07-01be6a2b194e) with a summary of findings and the path to your handoff.md.
