# Dispatch: explorer_fix_1

## Task
Investigate and design remediation plan for Phase M0 and M1 audit defects in `/home/jeff/aura/M0_M3_Audit_Report.md`.

## Evidence Files to Read
1. `/home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md`
2. `/home/jeff/aura/.agents/teamwork/auditor_audit_1/handoff.md`
3. `/home/jeff/aura/.agents/teamwork/auditor_audit_1/forensic_audit_report.md`
4. `/home/jeff/aura/.agents/teamwork/reviewer_audit_1/handoff.md`
5. `/home/jeff/aura/requirements.md`
6. `/home/jeff/aura/M0_M3_Audit_Report.md`

## Specific Defect Remediation Scope
1. `M0-SPIKE-03` & `HOST-002` (Tokio Runtime Pool):
   - Current status: Facade `init_tokio_pool()` in `aura-core/src/runtime/mod.rs:3-6`.
   - Action: Downgrade status from Complete to Partial. Formulate exact, concrete next steps.
2. `NET-007` (Stateless MCP Server over HTTP/3):
   - Current status: Empty UDP receive loop, no MCP JSON-RPC or `serverUrl`.
   - Action: Downgrade status to Partial. Formulate exact, concrete next steps.
3. `HOST-004` (5,000+ Concurrent Streams):
   - Current status: Verified only local port binding, not 5,000 streams.
   - Action: Downgrade to Partial. Formulate exact next steps.
4. `BUS-003` (Substrate dispatch simulation):
   - Current status: String formatting mock in `aura-core/src/bus/mod.rs:145-149`.
   - Action: Downgrade to Partial. Formulate exact next steps.
5. `HOST-011` (Component registry config):
   - Current status: Dummy `/tmp/mock.wasm` in `aura-core/src/runtime/registry.rs:33-43`.
   - Action: Downgrade to Partial. Formulate exact next steps.
6. `M0-SPIKE-02`, `AGT-001`, `AGT-006` line citation bounds:
   - File `aura-sdk/pyproject.toml` has 41 lines. Fix line ranges.
   - Also correct narrative on build system: uses `poetry-core` (`poetry.core.masonry.api`), not Hatchling.
7. `HOST-010`:
   - Replace directory citation `aura-core/src/` with specific entrypoint files.

Output your report to: `/home/jeff/aura/.agents/teamwork/explorer_fix_1/remediation_plan.md` and deliver `handoff.md`.


## 2026-10-10T01:09:21Z
You are explorer_fix_1, a teamwork_preview_explorer subagent.
Working directory: /home/jeff/aura/.agents/teamwork/explorer_fix_1/
Parent: Project Orchestrator (orchestrator_2)

Mandatory: Read /home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md first.
Read your task assignment: /home/jeff/aura/.agents/teamwork/explorer_fix_1/DISPATCH.md.
Read the full, unredacted forensic audit and reviewer handoffs:
- /home/jeff/aura/.agents/teamwork/auditor_audit_1/handoff.md
- /home/jeff/aura/.agents/teamwork/auditor_audit_1/forensic_audit_report.md
- /home/jeff/aura/.agents/teamwork/reviewer_audit_1/handoff.md

Your task:
Investigate the exact code in aura-core and aura-sdk to formulate a concrete, line-level remediation plan for Phase M0 & M1 defects in /home/jeff/aura/M0_M3_Audit_Report.md:
1. M0-SPIKE-03 & HOST-002: Evaluate aura-core/src/runtime/mod.rs:3-6. Propose status downgrade to Partial and concrete next steps.
2. NET-007: Evaluate aura-core/src/transport/http3.rs:8-10, 88-103. Propose status downgrade to Partial and concrete next steps.
3. HOST-004: Evaluate 5,000 stream scale claims vs test_quic_listener_binding. Propose status downgrade to Partial and concrete next steps.
4. BUS-003: Evaluate aura-core/src/bus/mod.rs:145-149. Propose status downgrade to Partial and concrete next steps.
5. HOST-011: Evaluate aura-core/src/runtime/registry.rs:33-43. Propose status downgrade to Partial and concrete next steps.
6. M0-SPIKE-02, AGT-001, AGT-006: Verify exact line length of aura-sdk/pyproject.toml (41 lines). Fix line bounds. Also correct narrative from Hatchling to Poetry build backend (poetry.core.masonry.api).
7. HOST-010: Replace directory citation aura-core/src/ with specific file citations.

Write your report to: /home/jeff/aura/.agents/teamwork/explorer_fix_1/remediation_plan.md
Deliver handoff.md and notify parent via send_message.
