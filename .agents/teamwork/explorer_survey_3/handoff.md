# Handoff Report: Survey Explorer 3 (AST Blast Radius & MCP Wiring)

**Date**: 2026-10-09  
**Sender**: Survey Explorer 3 (`explorer_survey_3`)  
**Recipient**: Orchestrator (`966c9504-d054-4c12-b035-b5bb1de49d34`)  
**Milestone**: M4 Repo Intelligence — `ast-blast-radius` MCP Wiring  
**Handoff Type**: Hard (Investigation complete)

---

## 1. Observation

1. **Crate Location and Workspace Configuration**:
   - `tools/ast-blast-radius` is located at `/home/jeff/aura/tools/ast-blast-radius/`.
   - Root `/home/jeff/aura/Cargo.toml` lines 6-8:
     ```toml
     exclude = [
         "tools"
     ]
     ```
   - Running `cargo test -p ast-blast-radius` from the repository root yields:
     ```
     error: package ID specification `ast-blast-radius` did not match any packages
     ```
   - Running `cargo test --manifest-path tools/Cargo.toml -p ast-blast-radius` or `cargo test --manifest-path tools/ast-blast-radius/Cargo.toml` succeeds:
     ```
     running 6 tests
     test test_blast_radius_safe_refactor ... ok
     test test_hybrid_search_bm25_matching ... ok
     test test_blast_radius_threshold_escalation ... ok
     test test_recursive_cte_cycle_safety ... ok
     test test_recursive_cte_dependency_traversal ... ok
     test test_submillisecond_execution ... ok

     test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     ```

2. **Crate Dependencies**:
   - In `/home/jeff/aura/tools/ast-blast-radius/Cargo.toml` lines 11-15:
     ```toml
     [dependencies]
     wit-bindgen = { workspace = true }
     serde = { workspace = true }
     serde_json = { workspace = true }
     ```
   - `serde` (with derive) and `serde_json` are already enabled as workspace dependencies in `/home/jeff/aura/tools/Cargo.toml`.

3. **Existing Crate Implementation**:
   - `src/lib.rs` (55 lines): Exports `cte`, `graph`, `risk`, `types` and implements WIT `aura:graph@1.0.0` world `blast-radius` via `exports::aura::graph::query::Guest for BlastRadiusComponent`.
   - `src/types.rs` (38 lines): Defines `SymbolInfo`, `RiskReport`, `BLAST_RADIUS_THRESHOLD = 10`, `MAX_SAFE_RISK_SCORE = 70`, `is_safe()`, and `requires_escalation()`.
   - `src/cte.rs` (106 lines): Implements BFS queue cycle-safe traversal simulating SQLite recursive CTE queries in `CallGraph::compute_reverse_dependencies`.
   - `src/risk.rs` (70 lines): Computes risk score, test coverage discount, and escalation penalty (+35) when affected count > 10.
   - `src/graph.rs` (242 lines): Implements `KnowledgeGraphEngine` with `seed_aura_core_symbols()`, `hybrid_search`, and `compute_blast_radius`.
   - `tests/blast_radius_test.rs` (134 lines): 6 passing unit tests.

4. **Missing MCP Component**:
   - There is currently no MCP parsing or dispatching module in `tools/ast-blast-radius`.
   - There is no unit test in `tests/blast_radius_test.rs` that tests routing a mock MCP request to the tool.

5. **Aura MCP Invariants and SDK Usage**:
   - `aura-architecture/references/04-mcp-http3-transport.md`: Specifies stateless July 2026 MCP spec over HTTP/3 with QUIC stream isolation.
   - `aura-sdk/aura_sdk/bus/client.py` lines 260-290: Invokes tools via `call_tool(tool_name, args)` with payload `{ "tool_id": cid, "tool_name": tool_name, "arguments": args }`.
   - `aura-sdk/aura_sdk/tools/ast_blast_radius.py` lines 8-15: Dispatches tool request with argument `{"target_symbol_id": target_symbol_id}` and checks if blast radius exceeds 10.

---

## 2. Logic Chain

1. **From Observation 1**: Because `tools` is excluded in root `Cargo.toml`, any `cargo test` command verifying `ast-blast-radius` must explicitly target `tools/Cargo.toml` or `tools/ast-blast-radius/Cargo.toml`.
2. **From Observations 2 & 4**: Because `serde` and `serde_json` are already present in `tools/ast-blast-radius/Cargo.toml`, implementing standard MCP JSON-RPC 2.0 handling does not require adding or updating external dependencies.
3. **From Observations 3 & 5**: The recursive CTE logic and threshold calculations (`affected_count > 10`, `risk_score >= 70`) are fully implemented and verified in `src/graph.rs`, `src/cte.rs`, and `src/risk.rs`. What remains for M4.2 is bridging MCP JSON-RPC 2.0 requests to `KnowledgeGraphEngine::compute_blast_radius` and returning the response in standard MCP response envelope.
4. **From Observation 5**: To ensure seamless interoperability between `wit/aura-graph.wit` (uses `symbol-id`), standard MCP conventions (uses `symbol_id`), and `aura-sdk/aura_sdk/tools/ast_blast_radius.py` (uses `target_symbol_id`), the MCP parameter extraction must accept either `"symbol_id"` or `"target_symbol_id"`.
5. **From Observations 1 & 4**: Implementing `src/mcp.rs` and exporting `handle_mcp_request` alongside dedicated test cases in `tests/blast_radius_test.rs` will directly fulfill requirement R2 and all objective verification criteria in `ORIGINAL_REQUEST.md`.

---

## 3. Caveats

- **Scope Boundary**: This survey investigated `tools/ast-blast-radius` and its MCP wiring requirements. It did not modify source code or implement the git stubs (which are being investigated by Explorer 1 and Explorer 2).
- **Transport Substrate**: The tool can be invoked either directly via in-process MCP handler in tests, or as a WASI component via stdio / QUIC stream. Implementing `handle_mcp_request(engine, request_str) -> String` provides full testability and straightforward integration with both execution models.

---

## 4. Conclusion

- `tools/ast-blast-radius` is fully functional in terms of CTE graph indexing and risk analysis, but completely lacks MCP JSON-RPC 2.0 wiring.
- A comprehensive survey report detailing the crate layout, MCP schemas (`tools/call` and `tools/list`), error handling, recursive CTE query evaluation, and proposed implementation code has been published to `/home/jeff/aura/.agents/teamwork/explorer_survey_3/survey_report.md`.
- Implementation strategy for milestone M4.2:
  1. Add `tools/ast-blast-radius/src/mcp.rs` containing `McpRequest`, `McpResponse`, `McpResult`, `McpContent`, `McpError`, `BlastRadiusMcpReport`, `McpHandler`, and `handle_mcp_request`.
  2. Re-export MCP types and `handle_mcp_request` in `tools/ast-blast-radius/src/lib.rs`.
  3. Add mock MCP routing unit tests to `tools/ast-blast-radius/tests/blast_radius_test.rs`.
  4. Verify with `cargo test --manifest-path tools/Cargo.toml -p ast-blast-radius`.

---

## 5. Verification Method

To verify the findings of this survey:
1. Inspect the survey report:
   ```bash
   view_file /home/jeff/aura/.agents/teamwork/explorer_survey_3/survey_report.md
   ```
2. Verify existing test suite behavior:
   ```bash
   cargo test --manifest-path tools/Cargo.toml -p ast-blast-radius
   ```
3. Verify `aura-core` baseline test suite:
   ```bash
   cargo test -p aura_core
   ```
