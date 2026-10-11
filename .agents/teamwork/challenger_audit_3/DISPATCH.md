# Dispatch: challenger_audit_3

## Role
Teamwork Preview Challenger (Automated Completeness & Citation Integrity)

## Target Artifact
`/home/jeff/aura/M0_M3_Audit_Report.md` and `/home/jeff/aura/requirements.md`

## Task
1. Execute an automated Python script that:
   - Extracts all requirement IDs from requirements.md (asserts exactly 121).
   - Extracts all requirement IDs from M0_M3_Audit_Report.md (asserts exactly 121).
   - Asserts 100% intersection.
   - Asserts status counts: exactly 110 Complete, 11 Partial, 0 Missing.
   - Asserts every Complete or Partial item has at least one valid path citation.
   - Asserts ALL cited file paths exist on disk (asserts 0 broken citations).
2. Report stats and deliver handoff.md with verdict: APPROVE or CHALLENGE_FAILED.

## 2026-10-10T01:26:09Z
You are challenger_audit_3, a teamwork_preview_challenger subagent.
Working directory: /home/jeff/aura/.agents/teamwork/challenger_audit_3/
Parent: Project Orchestrator (orchestrator_2)

Mandatory: Read /home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md first.
Read your dispatch: /home/jeff/aura/.agents/teamwork/challenger_audit_3/DISPATCH.md.

Scope: Empirical completeness cross-checking between /home/jeff/aura/requirements.md and /home/jeff/aura/M0_M3_Audit_Report.md.

Your task:
1. Write and execute an automated verification script (e.g. in python) that:
   - Parses /home/jeff/aura/requirements.md and extracts all requirement IDs (verify exactly 121).
   - Parses /home/jeff/aura/M0_M3_Audit_Report.md and extracts all requirement IDs (verify exactly 121).
   - Asserts 100% intersection.
   - Asserts status counts: exactly 110 Complete, 11 Partial, 0 Missing.
   - Asserts every Complete or Partial requirement has at least one cited file path.
   - Checks that all cited file paths in the report actually exist on disk in /home/jeff/aura/ (must be 0 broken citations).
2. Report exact counts, discrepancies (if any), and verification stats.
3. Deliver handoff.md with clear verdict: APPROVE or CHALLENGE_FAILED.
4. Notify parent via send_message.
