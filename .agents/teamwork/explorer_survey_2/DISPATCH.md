# Task Assignment: Survey Explorer 2 (Firecracker Sandbox Lint, Ephemeral Tunnel & Apply Patch)

- Working Directory: /home/jeff/aura/.agents/teamwork/explorer_survey_2/
- Parent Conversation ID: 966c9504-d054-4c12-b035-b5bb1de49d34
- User Request: /home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md

Investigate:
1. Locate Firecracker hypervisor module (`aura-core/src/hypervisor/firecracker.rs`) or related execution modules.
2. Investigate how `sandbox_lint` should launch Firecracker to run linters (ruff, prettier, cargo fmt) in a microVM, following R3 constraints.
3. Investigate `open_pr` and how Ephemeral Tunnel is used in the codebase to create PRs securely.
4. Investigate `apply_patch` and how it integrates `check_drift` and `sandbox_lint` before committing.
5. Review existing tests and test fixtures.
6. Relevant skills to inspect:
   - `/home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-exe-vm-domain/SKILL.md`
   - `/home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-repo-domain/SKILL.md`
   - `/home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-architecture/SKILL.md`

Produce a comprehensive survey report at `/home/jeff/aura/.agents/teamwork/explorer_survey_2/survey_report.md` and a summary in `handoff.md`.


## 2026-10-09T21:59:27Z
You are Survey Explorer 2 for the M4 Repo Intelligence task in Aura.
Your working directory is: /home/jeff/aura/.agents/teamwork/explorer_survey_2/
You must read ORIGINAL_REQUEST.md at: /home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md
Your task assignment is in: /home/jeff/aura/.agents/teamwork/explorer_survey_2/DISPATCH.md
Domain skills to inspect:
- /home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-exe-vm-domain/SKILL.md
- /home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-repo-domain/SKILL.md
- /home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-architecture/SKILL.md

Objectives:
1. Locate Firecracker hypervisor module (`aura-core/src/hypervisor/firecracker.rs`) or related microVM runtime files.
2. Investigate how `sandbox_lint` in `git.rs` should launch Firecracker to run linters (ruff, prettier, cargo fmt) in a microVM, satisfying requirement R3 (Controlled Execution).
3. Investigate `open_pr` and how the Ephemeral Tunnel is structured and used in Aura to create PRs securely.
4. Investigate `apply_patch` in `git.rs` and how it must integrate `check_drift` and `sandbox_lint` before committing.
5. Check existing tests in `aura-core` for patch application, linting, and hypervisor execution.
6. Document all findings, code locations, type definitions, function signatures, dependencies, and recommended implementation strategy in `/home/jeff/aura/.agents/teamwork/explorer_survey_2/survey_report.md`.
7. Write your handoff summary in `/home/jeff/aura/.agents/teamwork/explorer_survey_2/handoff.md` and message the orchestrator when complete.
