## 2026-10-10T00:46:13Z
You are worker_audit_synth, a teamwork_preview_worker subagent.
Working directory: /home/jeff/aura/.agents/teamwork/worker_audit_synth/
Parent: Project Orchestrator (orchestrator_2)

Mandatory: Read /home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md first.

DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

File write ownership:
You exclusively own:
- /home/jeff/aura/requirements.md
- /home/jeff/aura/M0_M3_Audit_Report.md

Your Task:
1. Ingest all 3 authoritative survey reports from the spec miners:
   - /home/jeff/aura/.agents/teamwork/spec_miner_survey_1/survey_report.md (Phase M0 Spikes & Phase M1 Kernel MVP)
   - /home/jeff/aura/.agents/teamwork/spec_miner_survey_2/survey_report.md (Phase M2 Sandboxes: WASM & Firecracker)
   - /home/jeff/aura/.agents/teamwork/spec_miner_survey_3/survey_report.md (Phase M3 Cognition: Interactions API, Router, Token Tracking, Context Saturation)

2. Create /home/jeff/aura/requirements.md:
   - The master canonical requirements document for the Aura project spanning phases M0 through M3.
   - Structure into clear sections:
     * Phase M0: Workspace Initialization & Core Primitives (M0-SPIKE-01..04)
     * Phase M1: Kernel MVP / Transport & Identity Federation (HOST-001..012, BUS-001..009, NET-001..007, IAM-001..008, AGT-001..006,009..011, NFR-SEC-001..004)
     * Phase M2: Execution Substrate / Docker-Free Sandboxing (EXE-WASM-001..008, EXE-VM-001..015, TOOL-CMP-001..004, TOOL-BASH-001..003, TOOL-BRW-001..003, TOOL-FS-001..004)
     * Phase M3: Cognitive Processing & Observability (COG-001..014, RTE-001..009, GAP-001..008)
   - Include all 121 requirements with their exact ID, name, full description, and authoritative source.

3. Create the final artifact /home/jeff/aura/M0_M3_Audit_Report.md:
   - Must fulfill all acceptance criteria from ORIGINAL_REQUEST.md:
     * Every requirement belonging to M0, M1, M2, and M3 from requirements.md is explicitly listed in the final report.
     * Every requirement has a clear Status assigned (Complete, Partial, Missing).
     * Any requirement marked "Complete" or "Partial" includes at least one specific file path and line number as a code citation.
     * Concrete, actionable next steps required to reach "Complete" for any requirement marked Partial or Missing.
   - Include Executive Summary, Comprehensive Status Matrix, Deep Dive per Phase, Gap Analysis Summary, and Verification Methods.

4. Run build and test verification:
   - Run `cargo test -p aura-core` and verify all tests pass.
   - Run `pytest aura-sdk/tests` and verify all tests pass.
   - Run `cargo test --workspace` if appropriate.
   - Document commands and results in your handoff report.

5. Deliver handoff.md and send completion message to parent.
