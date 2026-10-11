# Dispatch: reviewer_audit_4

## Role
Teamwork Preview Reviewer (Phase M2 & M3 Verification)

## Target Artifact
`/home/jeff/aura/M0_M3_Audit_Report.md` (specifically M2 and M3 sections: lines 136-256, and Section 4 items 7-11) against `/home/jeff/aura/requirements.md` and live codebase.

## Task
1. Verify that all 68 M2 and M3 requirements are listed with accurate statuses (M2: 33 Complete, 4 Partial; M3: 30 Complete, 1 Partial).
2. Confirm that EXE-VM-002 has valid file citations (no docs/ROADMAP.md) and accurate Partial status with concrete next steps.
3. Confirm that TOOL-CMP-003 has the deploy/images/ prefix.
4. Confirm that fabricated test strings in TOOL-CMP-002 and TOOL-BRW-002 have been removed and replaced with verified citations.
5. Confirm build_browser_image.sh line citation does not exceed 284 lines.
6. Run tests: `pytest aura-sdk/tests/test_cognition_gemini.py` and `cargo test --manifest-path tools/Cargo.toml`.
7. Deliver handoff.md with verdict: APPROVE or REQUEST_CHANGES.

## 2026-10-10T01:26:09Z

You are reviewer_audit_4, a teamwork_preview_reviewer subagent.
Working directory: /home/jeff/aura/.agents/teamwork/reviewer_audit_4/
Parent: Project Orchestrator (orchestrator_2)

Mandatory: Read /home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md first.
Read your dispatch: /home/jeff/aura/.agents/teamwork/reviewer_audit_4/DISPATCH.md.

Scope: Phase M2 (Sandboxes) and Phase M3 (Cognition) re-review of /home/jeff/aura/M0_M3_Audit_Report.md against /home/jeff/aura/requirements.md and the codebase.

Your task:
1. Examine /home/jeff/aura/M0_M3_Audit_Report.md for all 68 M2 and M3 requirements.
2. Confirm that EXE-VM-002 is marked Partial with valid citations (no docs/ROADMAP.md) and concrete next steps.
3. Confirm that TOOL-CMP-003 has the deploy/images/ path prefix and tests capture.rs:160-175.
4. Confirm fabricated test strings in TOOL-CMP-002 and TOOL-BRW-002 have been removed and replaced with verified test citations.
5. Confirm build_browser_image.sh citation does not exceed 284 lines.
6. Run tests: `/home/jeff/aura/venv/bin/pytest aura-sdk/tests/test_cognition_gemini.py` and `cargo test --manifest-path tools/Cargo.toml`.
7. Deliver handoff.md with clear verdict: APPROVE or REQUEST_CHANGES.
8. Notify parent via send_message.
