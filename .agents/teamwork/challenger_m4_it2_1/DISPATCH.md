# DISPATCH — challenger_m4_it2_1

## Mission
Empirically stress-test and challenge Milestone 4 Iteration 2 fixes for the persistent Bash tool and Firecracker session manager.

## Mandatory Inputs & Directives
- Read `/home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md`
- Read `/home/jeff/aura/AGENTS.md`
- Read `/home/jeff/aura/.agents/teamwork/orchestrator_9/PROJECT.md`
- Read `/home/jeff/aura/.agents/teamwork/worker_m4_fix/handoff.md`
- Working directory: `/home/jeff/aura/.agents/teamwork/challenger_m4_it2_1/`

## Focus
Adversarially verify the fixes for:
1. `aura-sdk/aura_sdk/tools/bash.py`:
   - Whitespace and special characters in working directory (`cd "{escaped_cwd}"`).
   - Unset environment variables and state continuity across sequential invocations.
   - Proper stderr capture on command failure without exit code masking.
2. `aura-core/src/hypervisor/firecracker.rs`:
   - Per-session lock concurrency (`Arc<tokio::sync::Mutex<BashSessionHandle>>`).
   - Clean shutdown and timeout handling.

## Verification Requirements
- Execute existing stress suites (`/home/jeff/aura/aura-sdk/.venv/bin/pytest aura-sdk/tests/test_bash_stress.py` and `cargo test -p aura-core --test m4_empirical_bash_session_stress_test`).
- Write and run any additional empirical stress probes as needed in your working directory.
- Provide clear verdict: APPROVE or REJECT in `/home/jeff/aura/.agents/teamwork/challenger_m4_it2_1/handoff.md`.
- Send completion message to parent.

## 2026-10-11T13:00:24Z
You are challenger_m4_it2_1, adversarial verification specialist for Milestone 4 Iteration 2 of the Aura project.
Your working directory is: /home/jeff/aura/.agents/teamwork/challenger_m4_it2_1/
Read your dispatch in: /home/jeff/aura/.agents/teamwork/challenger_m4_it2_1/DISPATCH.md
Read the user request in: /home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md
Read the constitution in: /home/jeff/aura/AGENTS.md
Read worker handoff in: /home/jeff/aura/.agents/teamwork/worker_m4_fix/handoff.md

Adversarially stress test the persistent Bash tool and Firecracker session manager:
- Whitespace and special chars in cwd
- Unset env variables and persistence across calls
- Exit code markers and stderr propagation
- Concurrent sessions locking isolation in Firecracker

Run:
- /home/jeff/aura/aura-sdk/.venv/bin/pytest aura-sdk/tests/test_bash_stress.py
- cargo test -p aura-core --test m4_empirical_bash_session_stress_test

Write your handoff report to /home/jeff/aura/.agents/teamwork/challenger_m4_it2_1/handoff.md with verdict APPROVE or REJECT.
Send completion message to parent.
