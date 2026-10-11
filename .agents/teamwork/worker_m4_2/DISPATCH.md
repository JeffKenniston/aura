# Task Assignment: Worker M4.2 (AST Blast Radius MCP Wiring)

- Working Directory: /home/jeff/aura/.agents/teamwork/worker_m4_2/
- Parent Conversation ID: 966c9504-d054-4c12-b035-b5bb1de49d34
- User Request: /home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md
- Project Spec: /home/jeff/aura/PROJECT.md
- Survey Report to read:
  - /home/jeff/aura/.agents/teamwork/explorer_survey_3/survey_report.md

## Exclusive Write Ownership
You exclusively own:
- `/home/jeff/aura/tools/ast-blast-radius/src/mcp.rs` (new file)
- `/home/jeff/aura/tools/ast-blast-radius/src/lib.rs` (re-export mcp module and functions)
- `/home/jeff/aura/tools/ast-blast-radius/tests/blast_radius_test.rs` (add unit tests)
Do NOT modify any other files unless strictly necessary for compilation.

## Mandatory Integrity Warning
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

## Requirements
Implement production-ready MCP wiring in `tools/ast-blast-radius`:
1. Implement `tools/ast-blast-radius/src/mcp.rs`:
   - Define JSON-RPC 2.0 structures: `McpRequest`, `McpResponse`, `McpResult`, `McpContent`, `McpError`.
   - Implement `McpHandler` and `handle_mcp_request(engine: &KnowledgeGraphEngine, request_json: &str) -> String`.
   - Support `tools/list` returning the `ast-blast-radius` tool definition and input schema.
   - Support `tools/call` for `ast-blast-radius`, extracting symbol ID from either `"symbol_id"` or `"target_symbol_id"`.
   - Call `engine.compute_blast_radius(symbol_id)` to execute the recursive CTE blast radius calculation.
   - Return formatted JSON report inside `McpContent::text`:
     - Must include: `target_symbol_id`, `affected_count`, `risk_score`, `requires_escalation`, `cycles_detected`, `transitively_affected_symbols`.
   - Return standard JSON-RPC errors for invalid JSON, unknown tool, missing symbol_id, or symbol not found in graph.
2. In `tools/ast-blast-radius/src/lib.rs`:
   - Declare `pub mod mcp;` and re-export `pub use mcp::{handle_mcp_request, McpHandler};` (or relevant types).
3. In `tools/ast-blast-radius/tests/blast_radius_test.rs`:
   - Add unit tests routing mock MCP JSON-RPC requests to `handle_mcp_request` and parsing the response:
     - Verify safe tool call produces valid MCP result with recursive CTE blast radius report.
     - Verify threshold escalation tool call (`affected_count > 10` or `risk_score >= 70`) returns `requires_escalation: true`.
     - Verify `tools/list` returns the registered tool schema.
     - Verify invalid JSON-RPC and unknown symbol return proper MCP errors.

## Verification Commands
Run and ensure clean exit (0 errors):
```bash
cargo test --manifest-path tools/Cargo.toml -p ast-blast-radius
```

Document your changes, build/test results, and handoff in `/home/jeff/aura/.agents/teamwork/worker_m4_2/handoff.md`.


## 2026-10-09T22:08:59Z
You are Worker M4.2 for the M4 Repo Intelligence task in Aura.
Your working directory is: /home/jeff/aura/.agents/teamwork/worker_m4_2/
You must read ORIGINAL_REQUEST.md at: /home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md
Your task assignment and exclusive file ownership are defined in: /home/jeff/aura/.agents/teamwork/worker_m4_2/DISPATCH.md
Project specification: /home/jeff/aura/PROJECT.md
Survey report with architecture blueprint:
- /home/jeff/aura/.agents/teamwork/explorer_survey_3/survey_report.md

Relevant skills:
- /home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-net-domain/SKILL.md
- /home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-repo-domain/SKILL.md
- /home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-architecture/SKILL.md

MANDATORY INTEGRITY WARNING:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

Your objective:
Implement the MCP wiring for `tools/ast-blast-radius` in `src/mcp.rs`, re-export in `src/lib.rs`, and add mock MCP routing unit tests in `tests/blast_radius_test.rs`.
Run `cargo test --manifest-path tools/Cargo.toml -p ast-blast-radius` and verify all tests pass (0 failures).
Write your completion report in `/home/jeff/aura/.agents/teamwork/worker_m4_2/handoff.md` and send a message when done.
