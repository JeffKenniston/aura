# Dispatch for Forensic Auditor M2

You are `auditor_m2_1` (role: Forensic Auditor, type: `teamwork_preview_auditor`).
Your assigned working directory is `/home/jeff/aura/.agents/teamwork/auditor_m2_1/`.

## Mandatory Reading:
- User Request: `/home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md`
- Master Baseline: `/home/jeff/aura/M0_M3_Audit_Report.md` (§ HOST-011, NET-007)
- Scope: `/home/jeff/aura/.agents/teamwork/orchestrator_7/PROJECT.md`
- Repository Constitution: `/home/jeff/aura/AGENTS.md` (Strict zero mock/stub policy)
- Worker M2 Handoff: `/home/jeff/aura/.agents/teamwork/worker_m2_2/handoff.md`

## Milestone 2 Files to Inspect & Audit:
1. `aura-core/src/runtime/registry.rs` (HOST-011)
2. `aura-core/src/transport/mcp.rs` (NET-007)
3. `aura-core/src/transport/mod.rs`
4. `aura-core/src/config/mod.rs`

## Mandatory Tasks:
1. Run all forensic integrity checks on the M2 implementation files.
2. Verify that there are ABSOLUTELY NO mocks, stubs, TODOs, or fake test results in production code (AGENTS.md).
3. Verify that `registry.rs` uses genuine Serde TOML manifest deserialization (`load_from_config`, `load_from_str`, `from_manifest`, `validate_file_paths`), and that the mock path check (`if path.contains("test")`) is completely removed.
4. Verify that `transport/mcp.rs` implements an authentic stateless JSON-RPC 2.0 MCP server handling `tools/list` and `tools/call`.
5. Execute `cargo test -p aura-core --lib runtime::registry::tests`
6. Execute `cargo test -p aura-core --lib transport::mcp::tests`
7. Execute `cargo test -p aura-core` (all 118 tests must pass).
8. Write your comprehensive audit report in `/home/jeff/aura/.agents/teamwork/auditor_m2_1/handoff.md`.
9. Conclude with a clear binary verdict: **CLEAN** or **INTEGRITY VIOLATION**.
10. Send a message to orchestrator with your verdict and findings.
