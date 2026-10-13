# BRIEFING — 2026-10-11T15:02:00Z

## Mission
Survey requirement R1 (Ephemeral Credential Lifecycle & Out-of-Band Secret Isolation) for Phase M5 (Hardening) of Aura.

## 🔒 My Identity
- Archetype: specification_miner
- Roles: [specification_miner, survey]
- Working directory: /home/jeff/aura/.agents/teamwork/teamwork_preview_spec_miner_survey_1
- Original parent: dae2eddf-f903-4d1f-b5f4-42a3e7019103
- Milestone: M5

## 🔒 Key Constraints
- Read-only specification miner: do NOT implement code changes.
- Discover and document features by probing the authoritative specification and codebase.
- No mocks, stubs, or TODOs. Code citations must be verified with exact file paths and line numbers.
- Write survey report to `/home/jeff/aura/.agents/teamwork/teamwork_preview_spec_miner_survey_1/survey_report.md` and handoff report to `/home/jeff/aura/.agents/teamwork/teamwork_preview_spec_miner_survey_1/handoff.md`.

## Current Parent
- Conversation ID: dae2eddf-f903-4d1f-b5f4-42a3e7019103
- Updated: not yet

## Loaded Skills
- **Source**: `/home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-iam-domain/SKILL.md`
  - **Local copy**: `/home/jeff/aura/.agents/teamwork/teamwork_preview_spec_miner_survey_1/skills/aura-iam-domain.md`
  - **Core methodology**: SPIFFE SVID workload identity, short TTL, authorized scopes, default deny, audit decisions.
- **Source**: `/home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-repo-domain/SKILL.md`
  - **Local copy**: `/home/jeff/aura/.agents/teamwork/teamwork_preview_spec_miner_survey_1/skills/aura-repo-domain.md`
  - **Core methodology**: Tree-sitter CST, rayon indexing, BLAKE3 drift detection, sandboxed CI linter hooks.
- **Source**: `/home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-tool-domain/SKILL.md`
  - **Local copy**: `/home/jeff/aura/.agents/teamwork/teamwork_preview_spec_miner_survey_1/skills/aura-tool-domain.md`
  - **Core methodology**: Ephemeral-Tunnel mints short-lived credentials; never inject into model context.
- **Source**: `/home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-architecture/SKILL.md`
  - **Local copy**: `/home/jeff/aura/.agents/teamwork/teamwork_preview_spec_miner_survey_1/skills/aura-architecture.md`
  - **Core methodology**: Microkernel decoupling, Zenoh bus IPC, Zero-Trust SPIFFE/SPIRE attestation.

## Task Summary
- **What to build**: Specification mining & gap survey for R1 of Phase M5 (Ephemeral-Tunnel capability, SPIFFE scope authorization, automatic revocation, GitManager::open_pr integration).
- **Success criteria**: Comprehensive survey report detailing exact features discovered, existing structures, what is already implemented, what is missing, interface contracts, file boundaries, and edge cases.
- **Interface contracts**: `tools/ephemeral-tunnel`, `aura-core/src/knowledge/git.rs`, `aura-core/src/identity/mod.rs`, `aura-core/src/runtime/registry.rs`.
- **Code layout**: Survey report and handoff in working directory.

## Key Decisions Made
- Completed deep forensic investigation across `tools/ephemeral-tunnel`, `aura-core/src/knowledge/git.rs`, `aura-core/src/identity/mod.rs`, `aura-core/src/hypervisor/firecracker.rs`, and test suites.
- Discovered 23 features and 19 edge cases across all four R1 domains.
- Identified critical security and functional gaps:
  1. `Credentials` lacks `Display`/`Debug` masking; raw tokens leak when formatted.
  2. `EphemeralTunnelComponent` in `lib.rs` instantiates a new service per call, losing revocation state.
  3. `GitManager::open_pr` only revokes on success; errors permanently leak tokens in `active_tokens`.
  4. Out-of-band injection into microVM sandboxes does not exist yet.
  5. `open_pr` hardcodes caller SVID rather than verifying caller's actual authorization.
- Delivered exhaustive `survey_report.md`.

## Artifact Index
- `/home/jeff/aura/.agents/teamwork/teamwork_preview_spec_miner_survey_1/DISPATCH.md` — Dispatch record
- `/home/jeff/aura/.agents/teamwork/teamwork_preview_spec_miner_survey_1/BRIEFING.md` — Situational awareness
- `/home/jeff/aura/.agents/teamwork/teamwork_preview_spec_miner_survey_1/progress.md` — Progress heartbeat
- `/home/jeff/aura/.agents/teamwork/teamwork_preview_spec_miner_survey_1/survey_report.md` — Survey report (complete)
- `/home/jeff/aura/.agents/teamwork/teamwork_preview_spec_miner_survey_1/handoff.md` — Handoff report (in progress)
