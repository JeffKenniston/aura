# DISPATCH — worker_m4_fix

## Mission
Apply the verified remediation fixes for Milestone 4 Iteration 2 across `aura-sdk` and `aura-core`, verify with the complete test suites, and deliver a production-grade handoff.

## Mandatory Inputs & Directives
- Read `/home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md`
- Read `/home/jeff/aura/AGENTS.md` (Zero mocks, stubs, or TODOs policy)
- Working directory: `/home/jeff/aura/.agents/teamwork/worker_m4_fix/`

## Mandatory Integrity Warning
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

## Write Ownership
You exclusively own edits to:
- `aura-sdk/aura_sdk/tools/bash.py`
- `aura-sdk/aura_sdk/tools/browser.py`
- `aura-sdk/tests/test_browser_tool_stress.py`
- `aura-core/src/hypervisor/firecracker.rs`

## Proposed Remediation Artifacts
1. **Bash Tool**:
   - Source: `/home/jeff/aura/.agents/teamwork/explorer_m4_fix_1/proposed_bash.py`
   - Target: `aura-sdk/aura_sdk/tools/bash.py`
2. **Browser Tool**:
   - Source: `/home/jeff/aura/.agents/teamwork/explorer_m4_fix_2/proposed_browser.py`
   - Target: `aura-sdk/aura_sdk/tools/browser.py`
3. **Browser Stress Tests**:
   - Patch: `/home/jeff/aura/.agents/teamwork/explorer_m4_fix_2/test_stress_remediation.patch`
   - Target: `aura-sdk/tests/test_browser_tool_stress.py`
4. **Firecracker MicroVM**:
   - Source: `/home/jeff/aura/.agents/teamwork/explorer_m4_fix_1/proposed_firecracker.rs` (or diff in `remediation.patch`)
   - Target: `aura-core/src/hypervisor/firecracker.rs`

## Verification Requirements
1. Run `pytest aura-sdk/tests` and confirm all 84/84 tests pass.
2. Run `cargo test -p aura-core` and confirm all tests pass cleanly.
3. Verify zero mocks/stubs/TODOs.
4. Record all test outputs and diffs in `/home/jeff/aura/.agents/teamwork/worker_m4_fix/handoff.md`.
5. Send completion message to parent when done.

## 2026-10-11T12:46:52Z
You are worker_m4_fix, the remediation worker for Milestone 4 Iteration 2 of the Aura project.
Your working directory is: /home/jeff/aura/.agents/teamwork/worker_m4_fix/
You must read your dispatch assignment in: /home/jeff/aura/.agents/teamwork/worker_m4_fix/DISPATCH.md
and the full user request in: /home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md
and the repository constitution in: /home/jeff/aura/AGENTS.md

MANDATORY INTEGRITY WARNING:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

Your task:
1. Apply the verified drop-in remediation fixes prepared by the exploration agents:
   - For `aura-sdk/aura_sdk/tools/bash.py`: Apply proposed code from `/home/jeff/aura/.agents/teamwork/explorer_m4_fix_1/proposed_bash.py`
   - For `aura-sdk/aura_sdk/tools/browser.py`: Apply proposed code from `/home/jeff/aura/.agents/teamwork/explorer_m4_fix_2/proposed_browser.py`
   - For `aura-sdk/tests/test_browser_tool_stress.py`: Apply patch `/home/jeff/aura/.agents/teamwork/explorer_m4_fix_2/test_stress_remediation.patch`
   - For `aura-core/src/hypervisor/firecracker.rs`: Apply the changes from `/home/jeff/aura/.agents/teamwork/explorer_m4_fix_1/proposed_firecracker.rs` (or `/home/jeff/aura/.agents/teamwork/explorer_m4_fix_1/remediation.patch`)
2. Verify:
   - Run `pytest aura-sdk/tests` and confirm all 84/84 tests pass.
   - Run `cargo test -p aura-core` and confirm all unit/integration tests pass.
3. Verify that there are zero mocks, stubs, or TODOs per AGENTS.md.
4. Write your full report and test output in `/home/jeff/aura/.agents/teamwork/worker_m4_fix/handoff.md`.
5. Send a completion message back to parent orchestrator.

## 2026-10-11T12:57:25Z
**Context**: Milestone 4 Iteration 2 remediation application
**Content**: Checking in on progress applying the fixes to bash.py, browser.py, test_browser_tool_stress.py, and firecracker.rs.
**Action**: Please report your current step, any obstacles encountered, and ETA for running tests and completing handoff.md.
