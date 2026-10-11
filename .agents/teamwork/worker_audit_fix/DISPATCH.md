# Dispatch: worker_audit_fix

## Task
Remediate `/home/jeff/aura/M0_M3_Audit_Report.md` by applying all findings and blueprints from:
- `/home/jeff/aura/.agents/teamwork/explorer_fix_1/remediation_plan.md`
- `/home/jeff/aura/.agents/teamwork/explorer_fix_2/remediation_plan.md`
- `/home/jeff/aura/.agents/teamwork/explorer_fix_3/remediation_plan.md`

## Write Ownership
You exclusively own:
- `/home/jeff/aura/M0_M3_Audit_Report.md`

## Mandatory Integrity Warning
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

## Execution Steps
1. Carefully read the three remediation plans above.
2. Update `/home/jeff/aura/M0_M3_Audit_Report.md`:
   - Executive Summary metrics table (110 Complete, 11 Partial, 0 Missing).
   - Status Matrix table rows:
     * Downgrade `M0-SPIKE-03`, `HOST-002`, `HOST-004`, `HOST-011`, `BUS-003`, `NET-007`, `EXE-VM-002` to `**Partial**` with concrete next steps.
     * Fix broken citations (`docs/ROADMAP.md:23` -> `aura-core/src/lib.rs:37`, `hypervisor/firecracker.rs:88-100`, `deploy/images/README.md:37`).
     * Fix missing prefix in `TOOL-CMP-003` (`deploy/images/vsock-screenshot-daemon/src/capture.rs:160-175`).
     * Remove fabricated test claims (`Bundled visual test scripts verified`, `Verified in cognitive sanitization pipeline`) and replace with verified citations.
     * Fix citation line overflows (`pyproject.toml:1-41`, `build_browser_image.sh:1-284`).
     * Fix minor line drift in `EXE-WASM-002`, `EXE-VM-005`, `TOOL-BASH-003`.
     * Replace directory citation `HOST-010` with specific file paths.
   - Section 3 Phase Summaries: update counts and Poetry backend narrative.
   - Section 4: expand to cover all 11 Partial requirements in full architectural detail with concrete next steps.
   - Section 5: update test commands to `cargo test --manifest-path tools/Cargo.toml` and `--manifest-path tools/filesystem/Cargo.toml`.
3. Run test verification:
   - `cargo test -p aura-core`
   - `/home/jeff/aura/venv/bin/pytest aura-sdk/tests`
   - `cargo test --manifest-path tools/Cargo.toml`
4. Deliver `handoff.md` and notify parent via `send_message`.

## 2026-10-10T01:18:23Z

Ingest all 3 comprehensive remediation plans:
1. /home/jeff/aura/.agents/teamwork/explorer_fix_1/remediation_plan.md (Phase M0 & M1 defect resolutions)
2. /home/jeff/aura/.agents/teamwork/explorer_fix_2/remediation_plan.md (Phase M2 defect resolutions)
3. /home/jeff/aura/.agents/teamwork/explorer_fix_3/remediation_plan.md (Systemic test commands, status rebalancing, Section 4 detailed analysis)

Carefully apply every remediation to /home/jeff/aura/M0_M3_Audit_Report.md:
1. Executive Summary:
   - Update metrics table: M0 (3 Complete, 1 Partial), M1 (44 Complete, 5 Partial), M2 (33 Complete, 4 Partial), M3 (30 Complete, 1 Partial); Total: 110 Complete (90.9%), 11 Partial (9.1%), 0 Missing (0.0%).
   - Enumerate all 11 Partial requirements.
2. Section 1 Status Matrix:
   - Downgrade M0-SPIKE-03, HOST-002, HOST-004, HOST-011, BUS-003, NET-007, EXE-VM-002 to **Partial** with concrete, actionable Next Steps.
   - Replace non-existent docs/ROADMAP.md:23 with aura-core/src/lib.rs:37, hypervisor/firecracker.rs:88-100, deploy/images/README.md:37.
   - Fix missing deploy/images/ prefix in TOOL-CMP-003 test citation (deploy/images/vsock-screenshot-daemon/src/capture.rs:160-175).
   - Remove fabricated test claims in TOOL-CMP-002 and TOOL-BRW-002 and replace with verified citations.
   - Fix line overflows in pyproject.toml (1-41) and build_browser_image.sh (1-284).
   - Correct minor line drift in EXE-WASM-002, EXE-VM-005, TOOL-BASH-003.
   - Replace directory citation HOST-010 with specific entrypoint files.
3. Section 3 Phase Summaries:
   - Update phase counts and note Poetry build backend (poetry.core.masonry.api) instead of Hatchling.
4. Section 4 Gap Analysis Deep Dive:
   - Expand Section 4 to include detailed architectural analyses and concrete numbered next steps for all 11 Partial requirements.
5. Section 5 Verification Commands:
   - Fix tools test commands to `cargo test --manifest-path tools/Cargo.toml` and `--manifest-path tools/filesystem/Cargo.toml`.
   - Document full passing test stats.
6. Run test suites:
   - `cargo test -p aura-core`
   - `/home/jeff/aura/venv/bin/pytest aura-sdk/tests`
   - `cargo test --manifest-path tools/Cargo.toml`
7. Deliver handoff.md and notify parent via send_message.

