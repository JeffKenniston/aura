# DISPATCH — worker_m5_verify

## Mission
Execute the comprehensive end-to-end workspace test suite verification across all crates and packages in Aura.

## Mandatory Inputs & Directives
- Read `/home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md`
- Read `/home/jeff/aura/AGENTS.md`
- Read `/home/jeff/aura/.agents/teamwork/orchestrator_9/PROJECT.md`
- Working directory: `/home/jeff/aura/.agents/teamwork/worker_m5_verify/`

## Verification Targets & Commands
1. **Python SDK Tests**:
   - `/home/jeff/aura/aura-sdk/.venv/bin/pytest aura-sdk/tests` (must pass 84/84)
   - `/home/jeff/aura/aura-sdk/.venv/bin/ruff check aura-sdk/aura_sdk aura-sdk/tests`
2. **Rust Core Tests**:
   - `cargo test -p aura-core --jobs 1` (must pass 146/146)
3. **AST Blast Radius Crate Tests**:
   - `cargo test --manifest-path tools/ast-blast-radius/Cargo.toml` (must pass 36/36)
4. **Vsock Shell Daemon Crate Tests**:
   - `cargo test --manifest-path deploy/images/vsock-shell-daemon/Cargo.toml` (must pass 8/8)
5. **Git Status & Working Tree Hygiene**:
   - `git status` check to ensure no dirty state or stray temp files in project root.

## Output
Write full outputs and summary to `/home/jeff/aura/.agents/teamwork/worker_m5_verify/handoff.md`.
Send completion message to parent orchestrator when done.


## 2026-10-11T13:11:10Z
From: 801dc836-0e0c-494b-8262-6981eb7e1ef0
Priority: MESSAGE_PRIORITY_HIGH
Content:
You are worker_m5_verify, full workspace verification worker for Milestone 5 of the Aura project.
Your working directory is: /home/jeff/aura/.agents/teamwork/worker_m5_verify/
Read your dispatch in: /home/jeff/aura/.agents/teamwork/worker_m5_verify/DISPATCH.md
Read the user request in: /home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md
Read the constitution in: /home/jeff/aura/AGENTS.md
Read project context in: /home/jeff/aura/.agents/teamwork/orchestrator_9/PROJECT.md

Your task:
1. Run /home/jeff/aura/aura-sdk/.venv/bin/pytest aura-sdk/tests and verify all 84/84 tests pass.
2. Run /home/jeff/aura/aura-sdk/.venv/bin/ruff check aura-sdk/aura_sdk aura-sdk/tests and verify clean.
3. Run cargo test -p aura-core --jobs 1 and verify all 146/146 tests pass.
4. Run cargo test --manifest-path tools/ast-blast-radius/Cargo.toml and verify all 36/36 tests pass.
5. Run cargo test --manifest-path deploy/images/vsock-shell-daemon/Cargo.toml and verify all 8/8 tests pass.
6. Check git status to ensure repository hygiene.
7. Write full report in /home/jeff/aura/.agents/teamwork/worker_m5_verify/handoff.md.
Send completion message to parent orchestrator.
