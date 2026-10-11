# Dispatch: explorer_fix_2

## Task
Investigate and design remediation plan for Phase M2 (Sandboxes) audit defects in `/home/jeff/aura/M0_M3_Audit_Report.md`.

## Evidence Files to Read
1. `/home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md`
2. `/home/jeff/aura/.agents/teamwork/auditor_audit_1/handoff.md`
3. `/home/jeff/aura/.agents/teamwork/auditor_audit_1/forensic_audit_report.md`
4. `/home/jeff/aura/.agents/teamwork/reviewer_audit_2/handoff.md`
5. `/home/jeff/aura/.agents/teamwork/challenger_audit_1/handoff.md`
6. `/home/jeff/aura/requirements.md`
7. `/home/jeff/aura/M0_M3_Audit_Report.md`

## Specific Defect Remediation Scope
1. `EXE-VM-002` (Nested Virtualization Host Validation):
   - Broken citation `/home/jeff/aura/docs/ROADMAP.md:23` does not exist on disk.
   - Facade implementation: `hypervisor::init_firecracker()` only prints a line and returns `Ok(())`; no `/dev/kvm` validation logic in `aura-core`.
   - Action: Downgrade to Partial. Replace citations with `aura-core/src/lib.rs:37` and `aura-core/src/hypervisor/firecracker.rs:88-100` (or `deploy/images/README.md:37`). Formulate concrete next steps (inspect `/dev/kvm` existence and read/write permissions).
2. `TOOL-CMP-003` (Rust AF_VSOCK Screenshot Daemon):
   - Test citation `vsock-screenshot-daemon/src/capture.rs:114-122` lacks the `deploy/images/` path prefix.
   - Action: Fix path to `deploy/images/vsock-screenshot-daemon/src/capture.rs:160-175` (test `test_encode_png_dummy_buffer`).
3. `TOOL-CMP-002`:
   - Remove fabricated test claim `Bundled visual test scripts verified`.
   - Replace with verified citations: `deploy/images/build_computer_image.sh:282-288` and `aura-sdk/aura_sdk/tools/computer.py:4-10`.
4. `TOOL-BRW-002`:
   - Remove fabricated test claim `Verified in cognitive sanitization pipeline`.
   - Replace with verified citations: prompt sanitization in `aura-sdk/aura_sdk/cognition/gemini.py:101-116`.
5. `TOOL-BRW-001`:
   - Fix citation line overflow: `deploy/images/build_browser_image.sh:1-300` -> file has 284 lines, fix to `1-284`.
6. Reviewer 2 minor line drift adjustments:
   - `EXE-WASM-002`: test citation to `/home/jeff/aura/aura-core/src/config/mod.rs:669-676`.
   - `EXE-VM-005`: test citation to `/home/jeff/aura/aura-core/src/config/mod.rs:570-590` and `firecracker.rs:596-614`.
   - `TOOL-BASH-003`: primary code citation to `/home/jeff/aura/aura-core/src/knowledge/git.rs:173-231`, test citation `git.rs:436-455, 521-549`.

Output your report to: `/home/jeff/aura/.agents/teamwork/explorer_fix_2/remediation_plan.md` and deliver `handoff.md`.


## 2026-10-10T01:09:21Z
You are explorer_fix_2, a teamwork_preview_explorer subagent.
Working directory: /home/jeff/aura/.agents/teamwork/explorer_fix_2/
Parent: Project Orchestrator (orchestrator_2)

Mandatory: Read /home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md first.
Read your task assignment: /home/jeff/aura/.agents/teamwork/explorer_fix_2/DISPATCH.md.
Read the full, unredacted forensic audit and reviewer handoffs:
- /home/jeff/aura/.agents/teamwork/auditor_audit_1/handoff.md
- /home/jeff/aura/.agents/teamwork/auditor_audit_1/forensic_audit_report.md
- /home/jeff/aura/.agents/teamwork/reviewer_audit_2/handoff.md
- /home/jeff/aura/.agents/teamwork/challenger_audit_1/handoff.md

Your task:
Investigate the exact code in deploy/images/, aura-core, tools/, and aura-sdk to formulate a concrete, line-level remediation plan for Phase M2 (Sandboxes) defects in /home/jeff/aura/M0_M3_Audit_Report.md:
1. EXE-VM-002: Resolve broken citation docs/ROADMAP.md:23 (file does not exist on disk). Inspect aura-core/src/lib.rs:37 and hypervisor/firecracker.rs:88-100 or deploy/images/README.md. Address /dev/kvm check facade in hypervisor/mod.rs:12-15: propose status downgrade to Partial and concrete next steps.
2. TOOL-CMP-003: Fix test citation vsock-screenshot-daemon/src/capture.rs:114-122 to include deploy/images/ prefix: deploy/images/vsock-screenshot-daemon/src/capture.rs:160-175.
3. TOOL-CMP-002: Remove fabricated test claim "Bundled visual test scripts verified". Replace with verified citations to deploy/images/build_computer_image.sh:282-288 and aura-sdk/aura_sdk/tools/computer.py:4-10.
4. TOOL-BRW-002: Remove fabricated test claim "Verified in cognitive sanitization pipeline". Replace with genuine prompt sanitization citation in aura-sdk/aura_sdk/cognition/gemini.py:101-116.
5. TOOL-BRW-001: Fix citation line overflow: deploy/images/build_browser_image.sh has 284 lines, adjust from 1-300 to 1-284.
6. Reviewer 2 minor line drift adjustments:
   - EXE-WASM-002: test citation to aura-core/src/config/mod.rs:669-676.
   - EXE-VM-005: test citation to aura-core/src/config/mod.rs:570-590 and firecracker.rs:596-614.
   - TOOL-BASH-003: primary citation to aura-core/src/knowledge/git.rs:173-231, test citation to git.rs:436-455, 521-549.

Write your report to: /home/jeff/aura/.agents/teamwork/explorer_fix_2/remediation_plan.md
Deliver handoff.md and notify parent via send_message.
