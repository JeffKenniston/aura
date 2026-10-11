## 2026-10-11T05:10:02Z
You are Explorer M4 Fix 1 investigating the remediation strategy for BashTool and Firecracker bash session concurrency.

Working Directory: `/home/jeff/aura/.agents/teamwork/explorer_m4_fix_1/`
Always create and maintain your `progress.md` and `BRIEFING.md` in your working directory.

MANDATORY INPUTS (READ FULL EVIDENCE):
1. `/home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md` (MANDATORY).
2. `/home/jeff/aura/.agents/teamwork/explorer_m4_fix_1/DISPATCH.md`.
3. `/home/jeff/aura/AGENTS.md`.
4. `/home/jeff/aura/.agents/teamwork/auditor_m4_1/handoff.md` (FULL FORENSIC AUDIT EVIDENCE REPORT).
5. `/home/jeff/aura/.agents/teamwork/challenger_m4_1/handoff.md` (FULL BASH STRESS REPORT).

Investigate and provide drop-in remediation code for:
- `aura-sdk/aura_sdk/tools/bash.py`:
  - Quoted cwd (`cd "{session['cwd']}"`)
  - Exit code capture and propagation via `__AURA_STATUS__=$?` and `__AURA_EXIT_MARKER__` (no masked exit codes, stderr returned on failure)
  - Full environment snapshot replacement so `unset` works
- `aura-core/src/hypervisor/firecracker.rs`:
  - Per-session locking or fine-grained concurrency in `BashSessionManager` so distinct sessions do not block each other.

Write your complete recommendations to `/home/jeff/aura/.agents/teamwork/explorer_m4_fix_1/handoff.md`.
Notify parent via send_message when done.
