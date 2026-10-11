# BRIEFING — 2026-10-11T03:02:01Z

## Mission
Implement Milestone 2 tasks: HOST-011 (authentic Serde TOML manifest loading in `registry.rs`) and NET-007 (stateless July 2026 MCP server interface over HTTP/3 in `transport/mcp.rs` and `transport/mod.rs`), verifying all unit and integration tests with zero warnings/failures and zero mocks.

## 🔒 My Identity
- Archetype: teamwork_preview_worker
- Roles: implementer, qa, specialist
- Working directory: /home/jeff/aura/.agents/teamwork/worker_m2_1/
- Original parent: b0e73c5b-179f-432b-9bbd-ebac6eea2996
- Milestone: Milestone 2 (Declarative Config & MCP HTTP/3)

## 🔒 Key Constraints
- Repository Constitution: Strict zero mock/stub policy (AGENTS.md). Code must be production-ready.
- Minimal change principle: only modify what is necessary.
- Build/Test commands: `cargo check --all-targets` and `cargo test -p aura-core`.
- .agents/teamwork/ contains only agent metadata.

## Current Parent
- Conversation ID: b0e73c5b-179f-432b-9bbd-ebac6eea2996
- Updated: 2026-10-11T03:02:01Z

## Task Summary
- **What to build**:
  1. HOST-011: Replace mock TOML parsing in `aura-core/src/runtime/registry.rs` with Serde TOML deserialization (`ToolManifestFile`, `ToolEntryConfig`, `load_from_config`, `load_from_str`, `from_manifest`, `validate_file_paths`), and unit tests parsing WASM and Firecracker tools.
  2. NET-007: Implement stateless MCP server in `aura-core/src/transport/mcp.rs` and export in `aura-core/src/transport/mod.rs`, handling JSON-RPC 2.0 `tools/list` and `tools/call`, with full unit/integration tests.
- **Success criteria**:
  - `cargo check --all-targets` passes without warnings.
  - `cargo test -p aura-core` passes 100% with zero failures.
  - No mocks or stubs in production logic.
- **Interface contracts**: `/home/jeff/aura/.agents/teamwork/orchestrator_7/PROJECT.md`
- **Code layout**: `/home/jeff/aura/aura-core/src/runtime/registry.rs`, `/home/jeff/aura/aura-core/src/transport/mcp.rs`, `/home/jeff/aura/aura-core/src/transport/mod.rs`

## Key Decisions Made
- [TBD]

## Artifact Index
- `/home/jeff/aura/.agents/teamwork/worker_m2_1/DISPATCH.md` — Assignment instructions
- `/home/jeff/aura/.agents/teamwork/worker_m2_1/BRIEFING.md` — Persistent agent memory
- `/home/jeff/aura/.agents/teamwork/worker_m2_1/progress.md` — Liveness heartbeat
- `/home/jeff/aura/.agents/teamwork/worker_m2_1/handoff.md` — Final handoff report

## Change Tracker
- **Files modified**: None yet
- **Build status**: Untested
- **Pending issues**: None

## Quality Status
- **Build/test result**: Untested
- **Lint status**: Clean
- **Tests added/modified**: None yet

## Loaded Skills
- **Source**: `/home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-host-domain/SKILL.md`
  - **Local copy**: `/home/jeff/aura/.agents/teamwork/worker_m2_1/skills/aura-host-domain.md`
  - **Core methodology**: Tokio async runtime, no blocking calls on worker threads, supervise child processes, graceful draining.
- **Source**: `/home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-net-domain/SKILL.md`
  - **Local copy**: `/home/jeff/aura/.agents/teamwork/worker_m2_1/skills/aura-net-domain.md`
  - **Core methodology**: HTTP/3 over QUIC, stateless July 2026 MCP spec, per-stream tool call mapping, TLS 1.3/SVID authentication.
