# BRIEFING — 2026-10-11T15:17:11Z

## Mission
Implement Milestone 1: ephemeral-tunnel credentials Display/Debug redaction and OnceLock state, GitManager scope validation & RAII EphemeralTokenGuard, and firecracker out-of-band credential injection & session timeout copy.

## 🔒 My Identity
- Archetype: worker
- Roles: implementer, qa, specialist
- Working directory: /home/jeff/aura/.agents/teamwork/worker_m1_1
- Original parent: dae2eddf-f903-4d1f-b5f4-42a3e7019103
- Milestone: Milestone 1

## 🔒 Key Constraints
- Exclusive write ownership over:
  - tools/ephemeral-tunnel/src/credentials.rs
  - tools/ephemeral-tunnel/src/lib.rs
  - tools/ephemeral-tunnel/src/spire.rs
  - tools/ephemeral-tunnel/tests/tunnel_test.rs
  - aura-core/src/knowledge/git.rs
  - aura-core/tests/m4_empirical_git_stress_test.rs
  - aura-core/src/hypervisor/firecracker.rs
- MANDATORY INTEGRITY WARNING: DO NOT CHEAT. All implementations must be genuine.
- Pass `cargo test --manifest-path tools/ephemeral-tunnel/Cargo.toml`
- Pass `cargo test -p aura-core --jobs 1`
- Zero regressions and zero compiler errors.
- Never write outside designated file write boundaries.

## Current Parent
- Conversation ID: dae2eddf-f903-4d1f-b5f4-42a3e7019103
- Updated: 2026-10-11T15:17:11Z

## Task Summary
- **What to build**: Milestone 1 implementation: ephemeral-tunnel credentials Display/Debug redaction, OnceLock state in lib.rs, unit tests; GitManager open_pr scope check & EphemeralTokenGuard RAII cleanup; out-of-band credential injection & session timeout state copying in firecracker.rs.
- **Success criteria**: Genuine implementation, cargo tests pass for ephemeral-tunnel and aura-core.
- **Interface contracts**: PROJECT.md, strategy reports from miner/explorer agents.
- **Code layout**: tools/ephemeral-tunnel and aura-core.

## Key Decisions Made
- [TBD]

## Artifact Index
- [TBD]

## Change Tracker
- **Files modified**: None yet
- **Build status**: Untested
- **Pending issues**: None

## Quality Status
- **Build/test result**: Untested
- **Lint status**: Untested
- **Tests added/modified**: TBD

## Loaded Skills
- TBD
