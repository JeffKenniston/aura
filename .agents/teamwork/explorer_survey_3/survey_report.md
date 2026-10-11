# Survey Report: AST Blast Radius & MCP Wiring (M4 Repo Intelligence)

**Date**: 2026-10-09  
**Explorer**: Survey Explorer 3 (`explorer_survey_3`)  
**Parent Task**: M4 Repo Intelligence — `ast-blast-radius` MCP Wiring  
**Integrity Mode**: Demo (Production-ready verification gates, zero mocks/stubs/TODOs)

---

## Executive Summary

Phase M4 (Repo Intelligence: Graph, Blast-Radius) requires wiring the `ast-blast-radius` tool to the Model Context Protocol (MCP) transport layer so autonomous agents can invoke blast-radius calculations over standard JSON-RPC 2.0 messages. Currently, `tools/ast-blast-radius` possesses fully operational in-memory and WebAssembly (WIT `aura:graph@1.0.0`) AST graph indexing and recursive Common Table Expression (CTE) dependency traversal logic. However, the crate lacks the MCP protocol translation layer (`McpRequest` / `McpResponse` serialization, dispatching, and tool execution schema), and lacks test coverage verifying mock MCP request routing.

This survey establishes the complete blueprint for wiring `ast-blast-radius` to MCP, defining the exact request/response schemas, error handling, recursive CTE query evaluation, and `cargo test` verification harness.

---

## 1. Crate Layout & Architecture of `tools/ast-blast-radius`

### 1.1 File Structure and Workspace Positioning
- **Absolute Path**: `/home/jeff/aura/tools/ast-blast-radius/`
- **Workspace Hierarchy**:
  - The root workspace (`/home/jeff/aura/Cargo.toml`) explicitly specifies `exclude = ["tools"]`.
  - The dedicated tools workspace is defined at `/home/jeff/aura/tools/Cargo.toml`, containing members:
    - `"filesystem"`
    - `"ast-blast-radius"`
    - `"ephemeral-tunnel"`
  - Testing must be invoked targeting the tool's manifest or the tools workspace:
    ```bash
    cargo test --manifest-path tools/Cargo.toml -p ast-blast-radius
    # OR
    cargo test --manifest-path tools/ast-blast-radius/Cargo.toml
    ```
- **Tool Metadata**: `/home/jeff/aura/tools/ast-blast-radius.json`
  ```json
  {
    "name": "ast-blast-radius",
    "executionEnvironment": "wasm-wasi-0.3",
    "category": "static-analysis",
    "description": "Recursive Common Table Expression dependency analysis against SQLite knowledge graph",
    "isolation": "wasm-memory-isolation"
  }
  ```

### 1.2 Dependencies in `tools/ast-blast-radius/Cargo.toml`
```toml
[package]
name = "ast-blast-radius"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
wit-bindgen = { workspace = true }  # 0.36
serde = { workspace = true }        # 1.0.229 (with derive features)
serde_json = { workspace = true }   # 1.0.151

[dev-dependencies]
tempfile = "3.10"
```
*Note*: `serde` (with derive) and `serde_json` are already workspace dependencies and immediately accessible for all MCP JSON-RPC structures without modifying dependencies.

### 1.3 Subsystem Files & Responsibilities
| File Path | Description / Key Types & Functions |
|---|---|
| `src/lib.rs` | World bindings for `aura:graph@1.0.0` export (`BlastRadiusComponent`), module declarations (`cte`, `graph`, `risk`, `types`), and re-exports. |
| `src/types.rs` | Core data structures: `SymbolInfo`, `RiskReport`, threshold constants `BLAST_RADIUS_THRESHOLD = 10`, `MAX_SAFE_RISK_SCORE = 70`, methods `is_safe()` and `requires_escalation()`. |
| `src/cte.rs` | In-memory recursive CTE traversal simulating SQLite recursive CTE queries: `ReverseDependency`, `CallGraph`, BFS queue traversal with shortest-depth tracking and cycle safety in `compute_reverse_dependencies`. |
| `src/risk.rs` | `calculate_risk_report`: Computes composite risk score (volume factor: `x5.5`, max depth factor: `x7.0`, direct caller density: `x3.5`, public API bonus: `+15`, threshold penalty: `+35` when affected count > 10, test coverage mitigation up to 45%). |
| `src/graph.rs` | `KnowledgeGraphEngine`: Index of `SymbolInfo`, call graph edges, test coverage registry, `hybrid_search`, `compute_blast_radius`, and `seed_aura_core_symbols()`. |
| `wit/aura-graph.wit` | WIT contract defining `aura:graph@1.0.0` interface `query` (`symbol-info`, `risk-report`, `hybrid-search`, `compute-blast-radius`). |
| `tests/blast_radius_test.rs` | Existing test suite containing 6 tests (BM25 matching, CTE traversal, cycle safety, safe refactor, threshold escalation, and submillisecond performance). All 6 currently pass. |

---

## 2. MCP Transport and Tool Registry in Aura

### 2.1 Architectural Invariants
Per `/home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-architecture/references/04-mcp-http3-transport.md` and `SKILL.md`:
1. **Stateless July 2026 MCP Spec**: Tools interface via the stateless Model Context Protocol using JSON-RPC 2.0 framing.
2. **Head-of-Line (HOL) Blocking Elimination via QUIC**: Tool invocations map to independent QUIC streams in `Http3Transport` (`aura-core/src/transport/http3.rs`).
3. **AST Blast Radius Pre-Check Boundary**: Per `06-semantic-cst-repository.md`, before applying structural refactorings or API changes, agents must execute recursive Common Table Expression queries against the knowledge graph. If blast radius > 10 downstream dependencies, refactoring must be halted and escalated to Gemini 3.1 Pro.

### 2.2 Existing Tool Invocations Across Aura Layers
1. **Host Runtime Registry** (`aura-core/src/runtime/registry.rs`):
   Maps tool names to `RuntimeBackend::Wasm(PathBuf)` or `RuntimeBackend::Firecracker(KernelConfig)`.
2. **Event Bus Client** (`aura-sdk/aura_sdk/bus/client.py`):
   `call_tool(tool_name, args)` transmits payload:
   ```json
   {
     "correlation_id": "<uuid>",
     "svid": "spiffe://...",
     "timestamp": 1234567.89,
     "schema_version": "1.0",
     "payload": {
       "tool_id": "<uuid>",
       "tool_name": "ast-blast-radius",
       "arguments": {
         "symbol_id": "aura_core::hypervisor::wasm::execute_component"
       }
     }
   }
   ```
3. **SDK Tool Client** (`aura-sdk/aura_sdk/tools/ast_blast_radius.py`):
   ```python
   class ASTBlastRadiusTool:
       async def analyze(self, target_symbol_id: str) -> str:
           result = await self.transport.dispatch_tool("ast-blast-radius", {"target_symbol_id": target_symbol_id})
           ...
           if count > 10:
               return f"CRITICAL: Blast radius is {count} (exceeds 10). Must escalate to Gemini 3.1 Pro."
   ```

---

## 3. Recursive CTE Blast Radius Logic

### 3.1 CTE Equivalence: SQL vs. In-Memory Graph
In SQLite (as utilized in `aura-core/src/knowledge/resolver.rs`):
```sql
WITH RECURSIVE downstream AS (
    SELECT to_symbol, 1 as depth FROM reference WHERE from_symbol = ?1
    UNION ALL
    SELECT r.to_symbol, d.depth + 1
    FROM reference r
    JOIN downstream d ON r.from_symbol = d.to_symbol
    WHERE d.depth < 10
)
SELECT COUNT(DISTINCT to_symbol) FROM downstream;
```

In `tools/ast-blast-radius/src/cte.rs`:
```sql
WITH RECURSIVE reverse_deps(source_id, target_id, depth) AS (
    SELECT caller_id, callee_id, 1 FROM call_graph WHERE callee_id = :symbol_id
    UNION
    SELECT cg.caller_id, cg.callee_id, rd.depth + 1
    FROM call_graph cg
    JOIN reverse_deps rd ON cg.callee_id = rd.source_id
    WHERE rd.depth < :max_depth
)
SELECT DISTINCT source_id, min(depth) FROM reverse_deps GROUP BY source_id;
```
The Rust implementation in `cte.rs` executes this using a cycle-safe breadth-first traversal up to `max_depth` (10), tracking the shortest path depth to each upstream caller and preventing infinite loops when cyclic dependencies exist.

### 3.2 Evaluation and Report Generation
1. When `compute_blast_radius(symbol_id)` is called:
   - It retrieves `SymbolInfo` for the symbol from `KnowledgeGraphEngine`. If not found, returns `Err("Symbol '<id>' not found in knowledge graph index")`.
   - Traverses the `CallGraph` to identify all transitive callers (`reverse_deps`).
   - Evaluates test coverage across affected symbols (`[target_symbol, ...reverse_deps]`).
   - Computes composite risk score in `calculate_risk_report`:
     - Base scoring on affected volume, call depth, and direct caller density.
     - Criticality bonus (+15) for traits and public APIs.
     - Threshold escalation penalty (+35) if affected symbols > 10.
     - Test coverage discount up to 45%.
2. Output struct `RiskReport`:
   ```rust
   pub struct RiskReport {
       pub affected_symbols: Vec<String>,
       pub risk_score: u32,
       pub test_coverage: f32,
   }
   ```
   - Threshold invariants:
     - `BLAST_RADIUS_THRESHOLD = 10`
     - `MAX_SAFE_RISK_SCORE = 70`
     - `is_safe()`: `len <= 10 && risk_score < 70`
     - `requires_escalation()`: `!is_safe()`

---

## 4. MCP Request Schema, Response Format & Tool Wiring

### 4.1 MCP JSON-RPC 2.0 Protocol Schema

#### Request Schema (`tools/call`)
```json
{
  "jsonrpc": "2.0",
  "id": "req-001",
  "method": "tools/call",
  "params": {
    "name": "ast-blast-radius",
    "arguments": {
      "symbol_id": "aura_core::hypervisor::wasm::execute_component"
    }
  }
}
```
*Note*: The parameter parser should accept either `"symbol_id"` or `"target_symbol_id"` to ensure backwards compatibility with `aura-sdk/aura_sdk/tools/ast_blast_radius.py`.

#### Successful Response Schema (`tools/call`)
Following the standard Model Context Protocol (July 2026 / 2024 spec):
```json
{
  "jsonrpc": "2.0",
  "id": "req-001",
  "result": {
    "content": [
      {
        "type": "text",
        "text": "{\n  \"target_symbol_id\": \"aura_core::hypervisor::wasm::execute_component\",\n  \"affected_count\": 3,\n  \"affected_symbols\": [\n    \"aura_core::hypervisor::wasm::execute_component\",\n    \"aura_cli::commands::run::execute\",\n    \"tests::integration::test_wasi_flow\"\n  ],\n  \"risk_score\": 38,\n  \"test_coverage\": 1.0,\n  \"is_safe\": true,\n  \"requires_escalation\": false\n}"
      }
    ],
    "isError": false
  }
}
```

#### Error Response Schema (`tools/call` for nonexistent symbol or invalid args)
```json
{
  "jsonrpc": "2.0",
  "id": "req-001",
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Symbol 'nonexistent::symbol' not found in knowledge graph index"
      }
    ],
    "isError": true
  }
}
```

#### Tool Discovery Schema (`tools/list`)
```json
{
  "jsonrpc": "2.0",
  "id": "req-002",
  "method": "tools/list"
}
```
Response:
```json
{
  "jsonrpc": "2.0",
  "id": "req-002",
  "result": {
    "tools": [
      {
        "name": "ast-blast-radius",
        "description": "Recursive Common Table Expression dependency analysis against SQLite knowledge graph",
        "inputSchema": {
          "type": "object",
          "properties": {
            "symbol_id": {
              "type": "string",
              "description": "Target symbol ID to evaluate AST blast radius for"
            }
          },
          "required": ["symbol_id"]
        }
      }
    ]
  }
}
```

---

## 5. Recommended Implementation Architecture for M4.2

### 5.1 Proposed Module: `tools/ast-blast-radius/src/mcp.rs`
Create a dedicated `mcp.rs` module encapsulating JSON-RPC 2.0 MCP data models and routing:

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::graph::KnowledgeGraphEngine;
use crate::types::RiskReport;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpRequest {
    pub jsonrpc: String,
    #[serde(default)]
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<McpResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResult {
    pub content: Vec<McpContent>,
    #[serde(rename = "isError")]
    pub is_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpError {
    pub code: i64,
    pub message: String,
}

/// Formatted report payload returned inside MCP content.text
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlastRadiusMcpReport {
    pub target_symbol_id: String,
    pub affected_count: usize,
    pub affected_symbols: Vec<String>,
    pub risk_score: u32,
    pub test_coverage: f32,
    pub is_safe: bool,
    pub requires_escalation: bool,
}

pub struct McpHandler<'a> {
    engine: &'a KnowledgeGraphEngine,
}

impl<'a> McpHandler<'a> {
    pub fn new(engine: &'a KnowledgeGraphEngine) -> Self {
        Self { engine }
    }

    pub fn handle_raw(&self, request_json: &str) -> String {
        let parsed: Result<McpRequest, _> = serde_json::from_str(request_json);
        match parsed {
            Ok(req) => {
                let resp = self.dispatch(req);
                serde_json::to_string(&resp).unwrap_or_else(|e| {
                    format!(
                        r#"{{"jsonrpc":"2.0","error":{{"code":-32603,"message":"Internal error: {}"}}}}"#,
                        e
                    )
                })
            }
            Err(e) => {
                let err_resp = McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id: None,
                    result: None,
                    error: Some(McpError {
                        code: -32700,
                        message: format!("Parse error: {}", e),
                    }),
                };
                serde_json::to_string(&err_resp).unwrap()
            }
        }
    }

    pub fn dispatch(&self, req: McpRequest) -> McpResponse {
        match req.method.as_str() {
            "tools/call" => self.handle_call_tool(req),
            "tools/list" => self.handle_list_tools(req),
            _ => McpResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id,
                result: None,
                error: Some(McpError {
                    code: -32601,
                    message: format!("Method not found: {}", req.method),
                }),
            },
        }
    }

    fn handle_call_tool(&self, req: McpRequest) -> McpResponse {
        let params = match req.params {
            Some(ref p) => p,
            None => {
                return McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id: req.id,
                    result: None,
                    error: Some(McpError {
                        code: -32602,
                        message: "Invalid params: missing parameters object".to_string(),
                    }),
                }
            }
        };

        let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if tool_name != "ast-blast-radius" {
            return McpResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id,
                result: None,
                error: Some(McpError {
                    code: -32601,
                    message: format!("Unknown tool: {}", tool_name),
                }),
            };
        }

        let arguments = params.get("arguments").and_then(|v| v.as_object());
        let symbol_id = arguments
            .and_then(|args| {
                args.get("symbol_id")
                    .or_else(|| args.get("target_symbol_id"))
            })
            .and_then(|v| v.as_str());

        let target_symbol = match symbol_id {
            Some(s) if !s.trim().is_empty() => s.trim(),
            _ => {
                return McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id: req.id,
                    result: Some(McpResult {
                        content: vec![McpContent {
                            content_type: "text".to_string(),
                            text: "Missing required argument 'symbol_id'".to_string(),
                        }],
                        is_error: true,
                    }),
                    error: None,
                }
            }
        };

        match self.engine.compute_blast_radius(target_symbol) {
            Ok(report) => {
                let mcp_report = BlastRadiusMcpReport {
                    target_symbol_id: target_symbol.to_string(),
                    affected_count: report.affected_symbols.len(),
                    affected_symbols: report.affected_symbols.clone(),
                    risk_score: report.risk_score,
                    test_coverage: report.test_coverage,
                    is_safe: report.is_safe(),
                    requires_escalation: report.requires_escalation(),
                };

                let report_json = serde_json::to_string_pretty(&mcp_report)
                    .unwrap_or_else(|_| format!("{:?}", report));

                McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id: req.id,
                    result: Some(McpResult {
                        content: vec![McpContent {
                            content_type: "text".to_string(),
                            text: report_json,
                        }],
                        is_error: false,
                    }),
                    error: None,
                }
            }
            Err(err) => McpResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id,
                result: Some(McpResult {
                    content: vec![McpContent {
                        content_type: "text".to_string(),
                        text: err,
                    }],
                    is_error: true,
                }),
                error: None,
            },
        }
    }

    fn handle_list_tools(&self, req: McpRequest) -> McpResponse {
        let tools_list = serde_json::json!({
            "tools": [
                {
                    "name": "ast-blast-radius",
                    "description": "Recursive Common Table Expression dependency analysis against SQLite knowledge graph",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "symbol_id": {
                                "type": "string",
                                "description": "Qualified symbol identifier to evaluate blast radius for"
                            }
                        },
                        "required": ["symbol_id"]
                    }
                }
            ]
        });

        // Directly construct response with tools object
        McpResponse {
            jsonrpc: "2.0".to_string(),
            id: req.id,
            result: Some(McpResult {
                content: vec![McpContent {
                    content_type: "text".to_string(),
                    text: serde_json::to_string_pretty(&tools_list).unwrap(),
                }],
                is_error: false,
            }),
            error: None,
        }
    }
}

/// Convenience function to route raw JSON-RPC string directly against the engine
pub fn handle_mcp_request(engine: &KnowledgeGraphEngine, request_json: &str) -> String {
    let handler = McpHandler::new(engine);
    handler.handle_raw(request_json)
}
```

### 5.2 Updates to `tools/ast-blast-radius/src/lib.rs`
1. Declare and export `mcp`:
   ```rust
   pub mod cte;
   pub mod graph;
   pub mod mcp;
   pub mod risk;
   pub mod types;

   pub use cte::{CallGraph, ReverseDependency};
   pub use graph::KnowledgeGraphEngine;
   pub use mcp::{handle_mcp_request, BlastRadiusMcpReport, McpHandler, McpRequest, McpResponse};
   pub use risk::calculate_risk_report;
   pub use types::{RiskReport, SymbolInfo};
   ```

### 5.3 Test Additions in `tools/ast-blast-radius/tests/blast_radius_test.rs`
Add test cases directly fulfilling Acceptance Criteria:
1. `test_mcp_blast_radius_safe_request()`:
   Routes mock request:
   ```json
   {
     "jsonrpc": "2.0",
     "id": "test-req-001",
     "method": "tools/call",
     "params": {
       "name": "ast-blast-radius",
       "arguments": {
         "symbol_id": "aura_core::hypervisor::wasm::execute_component"
       }
     }
   }
   ```
   Asserts:
   - Valid JSON returned with `jsonrpc == "2.0"` and `id == "test-req-001"`.
   - `result.isError == false`.
   - `result.content[0].text` deserializes into `BlastRadiusMcpReport`.
   - `report.target_symbol_id == "aura_core::hypervisor::wasm::execute_component"`.
   - `report.affected_count == 3` (safe).
   - `report.is_safe == true` and `report.requires_escalation == false`.

2. `test_mcp_blast_radius_threshold_escalation()`:
   Registers >10 callers for a target symbol in `KnowledgeGraphEngine`.
   Routes mock request.
   Asserts:
   - `result.isError == false`.
   - Deserialized report has `affected_count > 10`.
   - `report.requires_escalation == true` and `report.is_safe == false`.
   - `report.risk_score >= 70`.

3. `test_mcp_symbol_not_found()`:
   Routes request for unknown symbol `"nonexistent::func"`.
   Asserts:
   - `result.isError == true`.
   - `result.content[0].text` contains `"not found in knowledge graph index"`.

4. `test_mcp_tools_list()`:
   Routes `{"jsonrpc": "2.0", "id": 1, "method": "tools/list"}`.
   Asserts tool `ast-blast-radius` is listed with valid input schema.

---

## 6. Verification Plan

| Step | Command | Expected Result |
|---|---|---|
| 1. Unit Tests | `cargo test --manifest-path tools/Cargo.toml -p ast-blast-radius` | All 6 existing tests + new MCP mock routing tests pass (100% green). |
| 2. Manifest Target | `cargo test --manifest-path tools/ast-blast-radius/Cargo.toml` | Passes identical test suite with zero errors and clean output. |
| 3. Core Regression | `cargo test -p aura_core` | All 74 existing tests continue to pass without disruption. |

---

## 7. Conclusions & Next Steps for Orchestrator

1. **Self-Contained Implementation**: The crate `tools/ast-blast-radius` already has `serde` and `serde_json` dependencies and an active test harness. Implementing `src/mcp.rs` and adding tests to `tests/blast_radius_test.rs` will complete requirement R2 and all acceptance criteria for M4.2 with zero external crate churn.
2. **Backwards-Compatibility Guarantee**: The MCP argument parser should accept both `symbol_id` and `target_symbol_id` to cleanly align with `aura-sdk/aura_sdk/tools/ast_blast_radius.py` and `wit/aura-graph.wit`.
3. **Execution Command**: Ensure subsequent implementers execute `cargo test --manifest-path tools/Cargo.toml -p ast-blast-radius` (or `tools/ast-blast-radius/Cargo.toml`), since the root workspace excludes `tools/`.
