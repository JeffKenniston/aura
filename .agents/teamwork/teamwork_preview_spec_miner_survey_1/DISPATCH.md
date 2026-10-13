## 2026-10-11T14:46:44Z
Your identity is teamwork_preview_spec_miner_survey_1.
Your working directory is: /home/jeff/aura/.agents/teamwork/teamwork_preview_spec_miner_survey_1
You MUST read /home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md before starting work.

Domain skills to read if relevant:
- /home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-iam-domain/SKILL.md
- /home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-repo-domain/SKILL.md
- /home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-tool-domain/SKILL.md
- /home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-architecture/SKILL.md

Task:
Survey requirement R1 for Phase M5 (Hardening):
1. Ephemeral-Tunnel capability: minting short-lived, task-scoped credentials, injecting out-of-band directly into target execution sandboxes without exposing raw secrets to model context or agent memory (`[REDACTED-TOKEN-...]` opaque handles).
2. SPIFFE scope authorization checks (caller must be authorized for scope/service).
3. Automatic revocation on task completion, cancellation, or failure.
4. End-to-end pull request creation via `GitManager::open_pr` using ephemeral tokens.
5. Inspect existing codebase in `tools/ephemeral-tunnel`, `aura-core` (especially `git.rs`, `iam`, `registry`, etc.), and existing test suites.
6. Document exact features, existing structures, what is already implemented, what is missing, interface contracts, and file boundaries.

Write your comprehensive survey report to /home/jeff/aura/.agents/teamwork/teamwork_preview_spec_miner_survey_1/survey_report.md and your handoff to /home/jeff/aura/.agents/teamwork/teamwork_preview_spec_miner_survey_1/handoff.md. Send a completion message when done.
