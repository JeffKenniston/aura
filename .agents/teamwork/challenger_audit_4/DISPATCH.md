# Dispatch: challenger_audit_4

## Role
Teamwork Preview Challenger (Citation Validation & Workspace Test Suite Execution)

## Target Artifact
`/home/jeff/aura/M0_M3_Audit_Report.md` and live workspace

## Task
1. Sample at least 25 requirements across M0, M1, M2, and M3 from M0_M3_Audit_Report.md, specifically including:
   - Newly downgraded Partial items: M0-SPIKE-03, HOST-002, HOST-004, HOST-011, BUS-003, NET-007, EXE-VM-002
   - Corrected citation rows: TOOL-CMP-003, TOOL-CMP-002, TOOL-BRW-002, TOOL-BRW-001, AGT-001, AGT-006, M0-SPIKE-02, EXE-WASM-002, EXE-VM-005, TOOL-BASH-003
2. Verify cited source code at exact lines.
3. Execute all workspace test suites:
   - `cargo test -p aura-core`
   - `/home/jeff/aura/venv/bin/pytest aura-sdk/tests`
   - `cargo test --manifest-path tools/Cargo.toml`
   - `cargo test --manifest-path deploy/images/vsock-screenshot-daemon/Cargo.toml`
4. Confirm 100% tests pass and zero test failures.
5. Deliver handoff.md with verdict: APPROVE or CHALLENGE_FAILED.

## 2026-10-10T01:26:09Z
[Message] timestamp=2026-10-10T01:26:09Z sender=d71b58bc-d6bb-4674-ac7d-b24b5ce1f6c3 priority=MESSAGE_PRIORITY_HIGH content=You are challenger_audit_4, a teamwork_preview_challenger subagent.
Working directory: /home/jeff/aura/.agents/teamwork/challenger_audit_4/
Parent: Project Orchestrator (orchestrator_2)

Mandatory: Read /home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md first.
Read your dispatch: /home/jeff/aura/.agents/teamwork/challenger_audit_4/DISPATCH.md.

Scope: Empirical code citation validation and workspace test suite execution.

Your task:
1. Sample at least 25 random requirements across M0, M1, M2, and M3 from /home/jeff/aura/M0_M3_Audit_Report.md, including:
   - Downgraded Partial items: M0-SPIKE-03, HOST-002, HOST-004, HOST-011, BUS-003, NET-007, EXE-VM-002.
   - Remediated citation rows: TOOL-CMP-003, TOOL-CMP-002, TOOL-BRW-002, TOOL-BRW-001, AGT-001, AGT-006, M0-SPIKE-02, EXE-WASM-002, EXE-VM-005, TOOL-BASH-003.
2. Read the source files at the exact cited line numbers and verify that the cited code genuinely matches.
3. Execute the full workspace test suites:
   - `cargo test -p aura-core`
   - `/home/jeff/aura/venv/bin/pytest aura-sdk/tests`
   - `cargo test --manifest-path tools/Cargo.toml`
   - `cargo test --manifest-path deploy/images/vsock-screenshot-daemon/Cargo.toml`
4. Confirm 100% tests pass and zero test failures exist.
5. Deliver handoff.md with clear verdict: APPROVE or CHALLENGE_FAILED.
6. Notify parent via send_message.
