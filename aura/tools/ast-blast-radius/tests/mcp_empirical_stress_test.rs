use ast_blast_radius::{
    handle_mcp_request, KnowledgeGraphEngine, McpResponse, SymbolInfo,
};
use serde_json::Value;

#[test]
fn test_mcp_stress_malformed_json_rpc_requests() {
    let engine = KnowledgeGraphEngine::default();

    // 1. Completely invalid JSON syntax
    let malformed_syntaxes = vec![
        "",
        "   ",
        "{",
        "}",
        "{ broken json }",
        "{\"jsonrpc\": \"2.0\", \"method\": }",
        "{\"jsonrpc\": \"2.0\", \"id\": 1, \"method\": \"tools/call\", ",
        "<xml></xml>",
    ];

    for bad_json in malformed_syntaxes {
        let resp_str = handle_mcp_request(&engine, bad_json);
        let resp: McpResponse = serde_json::from_str(&resp_str)
            .unwrap_or_else(|e| panic!("Failed to parse response for '{}': {}", bad_json, e));
        assert_eq!(resp.jsonrpc, "2.0");
        assert!(resp.is_error());
        let err = resp.error.expect("Expected error object");
        assert_eq!(err.code, -32700, "Expected parse error code -32700 for input: {}", bad_json);
    }

    // 2. Valid JSON but invalid JSON-RPC structure (primitives, arrays, missing method)
    let bad_structures = vec![
        "42",
        "true",
        "\"a plain string\"",
        "[]",
        r#"{"jsonrpc": "2.0"}"#, // missing method
        r#"{"id": 1}"#,          // missing method and jsonrpc
    ];

    for bad_struct in bad_structures {
        let resp_str = handle_mcp_request(&engine, bad_struct);
        let resp: McpResponse = serde_json::from_str(&resp_str)
            .unwrap_or_else(|e| panic!("Failed to parse response for '{}': {}", bad_struct, e));
        assert_eq!(resp.jsonrpc, "2.0");
        assert!(resp.is_error());
    }

    // 3. Invalid params structures for tools/call
    let bad_params_payloads = vec![
        // Missing params altogether
        r#"{"jsonrpc": "2.0", "id": 1, "method": "tools/call"}"#,
        // params is null
        r#"{"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": null}"#,
        // params is string
        r#"{"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": "string"}"#,
        // params is array
        r#"{"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": [1, 2, 3]}"#,
        // params missing name
        r#"{"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {}}"#,
        // params arguments is number
        r#"{"jsonrpc": "2.0", "id": 6, "method": "tools/call", "params": {"name": "ast-blast-radius", "arguments": 123}}"#,
        // params arguments is null
        r#"{"jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": {"name": "ast-blast-radius", "arguments": null}}"#,
        // params arguments is array
        r#"{"jsonrpc": "2.0", "id": 8, "method": "tools/call", "params": {"name": "ast-blast-radius", "arguments": []}}"#,
    ];

    for payload in bad_params_payloads {
        let resp_str = handle_mcp_request(&engine, payload);
        let resp: McpResponse = serde_json::from_str(&resp_str)
            .unwrap_or_else(|e| panic!("Failed to parse response for payload: {}", e));
        assert_eq!(resp.jsonrpc, "2.0");
        assert!(resp.is_error(), "Payload should produce error response: {}", payload);
        let err = resp.error.expect("Expected McpError");
        assert!(
            err.code == -32602 || err.code == -32601,
            "Expected invalid params (-32602) or unknown tool (-32601), got {}",
            err.code
        );
    }
}

#[test]
fn test_mcp_stress_boundary_symbol_ids() {
    let engine = KnowledgeGraphEngine::default();

    // 1. Empty and whitespace-only symbol IDs
    let empty_symbol_payloads = vec![
        r#"{"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "ast-blast-radius", "arguments": {"symbol_id": ""}}}"#,
        r#"{"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "ast-blast-radius", "arguments": {"symbol_id": "   "}}}"#,
        r#"{"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "ast-blast-radius", "arguments": {"symbol_id": "\t\r\n"}}}"#,
        r#"{"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "ast-blast-radius", "arguments": {"target_symbol_id": ""}}}"#,
        r#"{"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {"name": "ast-blast-radius", "arguments": {"target_symbol_id": "   "}}}"#,
    ];

    for payload in empty_symbol_payloads {
        let resp_str = handle_mcp_request(&engine, payload);
        let resp: McpResponse = serde_json::from_str(&resp_str).unwrap();
        assert_eq!(resp.jsonrpc, "2.0");
        assert!(resp.is_error());
        let err = resp.error.unwrap();
        assert_eq!(err.code, -32602);
        assert!(err.message.contains("Missing required argument"));
    }

    // 2. Whitespace trimming around valid symbol
    let trimmed_payload = r#"{
        "jsonrpc": "2.0",
        "id": "trim-test",
        "method": "tools/call",
        "params": {
            "name": "ast-blast-radius",
            "arguments": {
                "symbol_id": "   aura_core::hypervisor::wasm::execute_component   "
            }
        }
    }"#;
    let resp_str = handle_mcp_request(&engine, trimmed_payload);
    let resp: McpResponse = serde_json::from_str(&resp_str).unwrap();
    assert!(resp.is_success(), "Trimmed valid symbol should succeed");
    let report = resp.parse_report().unwrap();
    assert_eq!(report.target_symbol_id, "aura_core::hypervisor::wasm::execute_component");

    // 3. Huge string symbol ID (10,000 characters) - must not panic, crash, or hang
    let huge_id = "a".repeat(10_000);
    let huge_payload = format!(
        r#"{{"jsonrpc":"2.0","id":99,"method":"tools/call","params":{{"name":"ast-blast-radius","arguments":{{"symbol_id":"{}"}}}}}}"#,
        huge_id
    );
    let resp_str = handle_mcp_request(&engine, &huge_payload);
    let resp: McpResponse = serde_json::from_str(&resp_str).unwrap();
    assert!(resp.is_error());
    let err = resp.error.unwrap();
    assert_eq!(err.code, -32004); // Symbol not found

    // 4. Special and injection characters (SQL injection, path traversal, unicode)
    let adversarial_symbols = vec![
        "'; DROP TABLE symbols; --",
        "../../../../etc/passwd",
        "🦀🔥✨_unicode_symbol",
        "sym\nwith\nnewlines",
    ];

    for sym in adversarial_symbols {
        let payload = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "adv-1",
            "method": "tools/call",
            "params": {
                "name": "ast-blast-radius",
                "arguments": {
                    "symbol_id": sym
                }
            }
        }).to_string();

        let resp_str = handle_mcp_request(&engine, &payload);
        let resp: McpResponse = serde_json::from_str(&resp_str).unwrap();
        assert!(resp.is_error());
        let err = resp.error.unwrap();
        assert_eq!(err.code, -32004);
        assert!(err.message.contains("not found in knowledge graph index"));
    }

    // 5. Varied id types (int, string, float, bool, null, missing)
    let ids = vec![
        serde_json::json!(1),
        serde_json::json!("string-id"),
        serde_json::json!(42.5),
        serde_json::json!(true),
        serde_json::Value::Null,
    ];

    for id_val in ids {
        let payload = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id_val,
            "method": "tools/call",
            "params": {
                "name": "ast-blast-radius",
                "arguments": {
                    "symbol_id": "aura_core::hypervisor::wasm::execute_component"
                }
            }
        }).to_string();

        let resp_str = handle_mcp_request(&engine, &payload);
        let resp: McpResponse = serde_json::from_str(&resp_str).unwrap();
        assert!(resp.is_success());
        assert_eq!(resp.id.unwrap_or(Value::Null), id_val);
    }
}

#[test]
fn test_mcp_stress_deeply_nested_and_adversarial_graphs() {
    let mut engine = KnowledgeGraphEngine::new();

    // 1. Deep linear chain of 50 nodes: node_50 -> node_49 -> ... -> node_0
    // Target is node_0.
    for i in 0..=50 {
        let sym_id = format!("chain::node_{:02}", i);
        engine.register_symbol(
            SymbolInfo {
                id: sym_id.clone(),
                file_id: format!("chain/{}.rs", i),
                kind: if i == 0 { "interface".to_string() } else { "function".to_string() },
                name: format!("node_{:02}", i),
                qualified_name: sym_id.clone(),
                signature: format!("fn node_{:02}()", i),
            },
            true,
        );
        if i > 0 {
            let caller = format!("chain::node_{:02}", i);
            let callee = format!("chain::node_{:02}", i - 1);
            engine.register_call(&caller, &callee);
        }
    }

    let payload = r#"{
        "jsonrpc": "2.0",
        "id": "deep-chain",
        "method": "tools/call",
        "params": {
            "name": "ast-blast-radius",
            "arguments": {
                "symbol_id": "chain::node_00"
            }
        }
    }"#;

    let resp_str = handle_mcp_request(&engine, payload);
    let resp: McpResponse = serde_json::from_str(&resp_str).unwrap();
    assert!(resp.is_success());

    let report = resp.parse_report().unwrap();
    assert_eq!(report.target_symbol_id, "chain::node_00");

    // Max depth cap is 10; depth 1 to 10 are visited (nodes 01 through 10)
    // Target (node_00) + 10 reverse deps = 11 affected symbols total
    assert_eq!(report.affected_count, 11);
    assert_eq!(report.transitively_affected_symbols.len(), 10);
    // Depth > 10 (node_11..node_50) must NOT be traversed due to max_depth cap
    assert!(!report.affected_symbols.contains(&"chain::node_11".to_string()));
    assert!(!report.affected_symbols.contains(&"chain::node_50".to_string()));
    assert!(report.requires_escalation, "11 symbols > threshold of 10 triggers escalation");

    // 2. Wide star graph: 80 direct callers
    let mut star_engine = KnowledgeGraphEngine::new();
    let center_id = "star::center_hub";
    star_engine.register_symbol(
        SymbolInfo {
            id: center_id.to_string(),
            file_id: "star/hub.rs".to_string(),
            kind: "public_api".to_string(),
            name: "center_hub".to_string(),
            qualified_name: center_id.to_string(),
            signature: "pub fn center_hub()".to_string(),
        },
        false,
    );

    for i in 0..80 {
        let caller_id = format!("star::caller_{:02}", i);
        star_engine.register_symbol(
            SymbolInfo {
                id: caller_id.clone(),
                file_id: format!("star/caller_{}.rs", i),
                kind: "function".to_string(),
                name: format!("caller_{:02}", i),
                qualified_name: caller_id.clone(),
                signature: format!("fn caller_{:02}()", i),
            },
            false,
        );
        star_engine.register_call(&caller_id, center_id);
    }

    let star_payload = r#"{
        "jsonrpc": "2.0",
        "id": "star-test",
        "method": "tools/call",
        "params": {
            "name": "ast-blast-radius",
            "arguments": {
                "symbol_id": "star::center_hub"
            }
        }
    }"#;

    let star_resp_str = handle_mcp_request(&star_engine, star_payload);
    let star_resp: McpResponse = serde_json::from_str(&star_resp_str).unwrap();
    assert!(star_resp.is_success());
    let star_report = star_resp.parse_report().unwrap();
    assert_eq!(star_report.affected_count, 81); // center + 80 callers
    assert!(star_report.requires_escalation);
    assert!(!star_report.is_safe);

    // 3. Dense cyclic mesh with self-loops
    let mut mesh_engine = KnowledgeGraphEngine::new();
    let mesh_nodes = ["mesh::A", "mesh::B", "mesh::C", "mesh::D"];
    for &node in &mesh_nodes {
        mesh_engine.register_symbol(
            SymbolInfo {
                id: node.to_string(),
                file_id: "mesh/mod.rs".to_string(),
                kind: "function".to_string(),
                name: node.to_string(),
                qualified_name: node.to_string(),
                signature: format!("fn {}()", node),
            },
            true,
        );
    }

    // A calls B, B calls C, C calls D, D calls A (cycle)
    mesh_engine.register_call("mesh::A", "mesh::B");
    mesh_engine.register_call("mesh::B", "mesh::C");
    mesh_engine.register_call("mesh::C", "mesh::D");
    mesh_engine.register_call("mesh::D", "mesh::A");
    // Self-loop: B calls B
    mesh_engine.register_call("mesh::B", "mesh::B");
    // Cross edges: C calls A, D calls B
    mesh_engine.register_call("mesh::C", "mesh::A");
    mesh_engine.register_call("mesh::D", "mesh::B");

    let mesh_payload = r#"{
        "jsonrpc": "2.0",
        "id": "mesh-test",
        "method": "tools/call",
        "params": {
            "name": "ast-blast-radius",
            "arguments": {
                "symbol_id": "mesh::B"
            }
        }
    }"#;

    let mesh_resp_str = handle_mcp_request(&mesh_engine, mesh_payload);
    let mesh_resp: McpResponse = serde_json::from_str(&mesh_resp_str).unwrap();
    assert!(mesh_resp.is_success());
    let mesh_report = mesh_resp.parse_report().unwrap();
    // B has reverse dependencies: A, C, D (and B should not be duplicated)
    assert!(mesh_report.affected_count <= 4);
    assert_eq!(mesh_report.target_symbol_id, "mesh::B");
}

#[test]
fn test_mcp_standard_format_schema_conformance() {
    let engine = KnowledgeGraphEngine::default();

    // 1. Verify tools/call schema conformance
    let call_req = r#"{
        "jsonrpc": "2.0",
        "id": "schema-verify-01",
        "method": "tools/call",
        "params": {
            "name": "ast-blast-radius",
            "arguments": {
                "symbol_id": "aura_core::hypervisor::wasm::execute_component"
            }
        }
    }"#;

    let raw_resp = handle_mcp_request(&engine, call_req);
    let val: serde_json::Value = serde_json::from_str(&raw_resp).unwrap();

    // Check top-level JSON-RPC 2.0 fields
    assert_eq!(val["jsonrpc"], "2.0");
    assert_eq!(val["id"], "schema-verify-01");
    assert!(val.get("error").is_none());
    assert!(val["result"].is_object());

    // Check MCP Result fields
    let result = &val["result"];
    assert_eq!(result["isError"], false);
    assert!(result["content"].is_array());
    assert_eq!(result["content"].as_array().unwrap().len(), 1);

    // Check Content element
    let content0 = &result["content"][0];
    assert_eq!(content0["type"], "text");
    assert!(content0["text"].is_string());

    // Parse text as BlastRadiusMcpReport and verify all required report fields
    let report_val: serde_json::Value = serde_json::from_str(content0["text"].as_str().unwrap()).unwrap();
    assert!(report_val["target_symbol_id"].is_string());
    assert!(report_val["affected_count"].is_number());
    assert!(report_val["affected_symbols"].is_array());
    assert!(report_val["transitively_affected_symbols"].is_array());
    assert!(report_val["risk_score"].is_number());
    assert!(report_val["test_coverage"].is_number());
    assert!(report_val["is_safe"].is_boolean());
    assert!(report_val["requires_escalation"].is_boolean());
    assert!(report_val["cycles_detected"].is_boolean());

    // 2. Verify tools/list schema conformance
    let list_req = r#"{
        "jsonrpc": "2.0",
        "id": "schema-list-01",
        "method": "tools/list"
    }"#;

    let list_raw = handle_mcp_request(&engine, list_req);
    let list_val: serde_json::Value = serde_json::from_str(&list_raw).unwrap();

    assert_eq!(list_val["jsonrpc"], "2.0");
    assert_eq!(list_val["id"], "schema-list-01");
    assert!(list_val["result"]["tools"].is_array());

    let tools = list_val["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 1);
    let tool = &tools[0];
    assert_eq!(tool["name"], "ast-blast-radius");
    assert!(tool["description"].is_string());
    assert!(tool["inputSchema"].is_object());
    assert_eq!(tool["inputSchema"]["type"], "object");
    assert!(tool["inputSchema"]["properties"]["symbol_id"].is_object());
    assert_eq!(tool["inputSchema"]["required"][0], "symbol_id");
}
