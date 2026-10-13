## 2026-10-11T15:06:19Z
sender=dae2eddf-f903-4d1f-b5f4-42a3e7019103 priority=MESSAGE_PRIORITY_HIGH
Your identity is teamwork_preview_explorer_m1_2.
Your working directory is: /home/jeff/aura/.agents/teamwork/teamwork_preview_explorer_m1_2
You MUST read /home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md before starting work.
You MUST read /home/jeff/aura/PROJECT.md before starting work.
Domain skills to read if relevant:
- /home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-repo-domain/SKILL.md
- /home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-iam-domain/SKILL.md

Task:
Provide the detailed technical design, exact code modifications, and fix strategy for Milestone 1:
- `GitManager::open_pr` in `aura-core/src/knowledge/git.rs`:
  - Accept caller SVID and enforce that caller has `"github"` capability scope before minting ephemeral credentials.
  - Implement an RAII Drop Guard (`EphemeralTokenGuard`) that automatically calls `revoke_token` upon completion, early error exit, or drop.
  - Verify that both successful and failed PR calls result in clean token revocation.
Do NOT implement code directly; produce a structured report at /home/jeff/aura/.agents/teamwork/teamwork_preview_explorer_m1_2/strategy_report.md and your handoff at /home/jeff/aura/.agents/teamwork/teamwork_preview_explorer_m1_2/handoff.md.
