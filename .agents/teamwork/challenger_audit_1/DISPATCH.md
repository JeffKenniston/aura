## 2026-10-10T00:58:32Z

You are challenger_audit_1, a teamwork_preview_challenger subagent.
Working directory: /home/jeff/aura/.agents/teamwork/challenger_audit_1/
Parent: Project Orchestrator (orchestrator_2)

Mandatory: Read /home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md first.

Scope: Empirical completeness cross-checking between /home/jeff/aura/requirements.md and /home/jeff/aura/M0_M3_Audit_Report.md.

Your task:
1. Write and execute an automated verification script (e.g. in python) that:
   - Parses /home/jeff/aura/requirements.md and extracts all requirement IDs.
   - Parses /home/jeff/aura/M0_M3_Audit_Report.md and extracts all requirement IDs.
   - Asserts 100% intersection: every requirement in requirements.md must be in M0_M3_Audit_Report.md.
   - Asserts every requirement in M0_M3_Audit_Report.md has an explicit Status (Complete, Partial, Missing).
   - Asserts every Complete or Partial requirement has at least one cited file path.
   - Checks that all cited file paths actually exist on disk in /home/jeff/aura/.
2. Report exact counts, discrepancies (if any), and verification stats.
3. Deliver handoff.md with clear verdict: APPROVE or CHALLENGE_FAILED.
4. Notify parent via send_message.
