# BRIEFING — 2026-10-11T14:56:00Z

## Mission
Survey requirements R3 (Microkernel Zero-Trust Security & Storage Encryption) and R4 (SPIRE Identity Federation & Transport Fallback Resilience) for Phase M5 (Hardening).

## 🔒 My Identity
- Archetype: explorer
- Roles: investigation, synthesis
- Working directory: /home/jeff/aura/.agents/teamwork/teamwork_preview_explorer_survey_3
- Original parent: dae2eddf-f903-4d1f-b5f4-42a3e7019103
- Milestone: M5 (Hardening)

## 🔒 Key Constraints
- Read-only investigation — do NOT implement
- Zero mocks, stubs, or TODOs in production code
- Write only to working directory /home/jeff/aura/.agents/teamwork/teamwork_preview_explorer_survey_3
- All reports must be self-contained with complete evidence chains

## Current Parent
- Conversation ID: dae2eddf-f903-4d1f-b5f4-42a3e7019103
- Updated: 2026-10-11T14:47:00Z

## Investigation State
- **Explored paths**:
  - `aura-core/src/hypervisor/firecracker.rs` (vsock streaming & listeners)
  - `aura-core/src/hypervisor/proxy.rs` (transparent TLS proxy & registry allowlisting)
  - `aura-core/src/config/mod.rs` (hypervisor & transport configs)
  - `aura-core/src/knowledge/cache.rs` (SQLite vector database)
  - `aura-core/src/identity/mod.rs` (SPIFFE identity, FederationManager, MtlsEnforcer)
  - `aura-core/src/transport/http3.rs`, `mcp.rs`, `mod.rs` (HTTP/3, TCP fallback, MCP server)
  - Entire test suite: `cargo test -p aura-core --jobs 1` (146 tests), SDK pytest (84 tests), ruff
- **Key findings**:
  - R3.1: `start_screenshot_listener` in `firecracker.rs` reads unbounded bytes via `read_to_end`; needs `VsockStreamConfig` and instant stream truncation.
  - R3.2: `EgressProxy` in `proxy.rs` already implements SNI inspection, package registry allowlisting, and TLS Fatal Alert 49 (Access Denied); needs module re-export and microVM lifecycle attachment hook.
  - R3.3: Storage encryption at rest is missing; needs `storage` module with `EncryptedDatabase` (AES-256-GCM + rusqlite), `TaskLedger`, and SPIFFE SVID key derivation (`derive_storage_key`).
  - R4.1: SPIRE trust domain federation manager exists in `identity/mod.rs`, but enterprise OIDC token parsing/verification and federation claim-to-scope mappings are missing.
  - R4.2: `Http3Transport` TCP listener does not execute MCP; needs `McpServer` wiring and a resilient client that detects UDP failure within 1 RTT (<100ms) and fails over silently to TCP without dropping in-flight tool calls.
- **Unexplored areas**: None. Survey is complete.

## Key Decisions Made
- Fully documented exact interfaces, data models, file boundaries, and verification suites in `survey_report.md`.
- Authored 5-component self-contained handoff report in `handoff.md`.

## Artifact Index
- `/home/jeff/aura/.agents/teamwork/teamwork_preview_explorer_survey_3/DISPATCH.md` — Recorded dispatch instructions
- `/home/jeff/aura/.agents/teamwork/teamwork_preview_explorer_survey_3/BRIEFING.md` — Situational awareness working memory
- `/home/jeff/aura/.agents/teamwork/teamwork_preview_explorer_survey_3/progress.md` — Heartbeat and activity log
- `/home/jeff/aura/.agents/teamwork/teamwork_preview_explorer_survey_3/survey_report.md` — Comprehensive survey report
- `/home/jeff/aura/.agents/teamwork/teamwork_preview_explorer_survey_3/handoff.md` — 5-component self-contained handoff report
