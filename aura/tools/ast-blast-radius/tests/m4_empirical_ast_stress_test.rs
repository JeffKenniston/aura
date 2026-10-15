use ast_blast_radius::{
    handle_mcp_request, CallGraph, KnowledgeGraphEngine, McpResponse, SymbolInfo,
};
use serde_json::json;

/// Test helper to create a minimal SymbolInfo for testing
fn create_test_symbol(id: &str, kind: &str) -> SymbolInfo {
    SymbolInfo {
        id: id.to_string(),
        file_id: format!("{}.rs", id),
        kind: kind.to_string(),
        name: id.to_string(),
        qualified_name: id.to_string(),
        signature: format!("pub fn {}()", id),
    }
}

// =========================================================================
// 1. RECURSIVE CTE CYCLE SAFETY UNDER ADVERSARIAL TOPOLOGIES
// =========================================================================

#[test]
fn test_adversarial_self_loop_cycle() {
    let mut graph = CallGraph::new();
    // Self-loop on target: target calls target
    graph.add_call_edge("target", "target");
    // Self-loop on caller: caller calls caller and calls target
    graph.add_call_edge("caller_1", "caller_1");
    graph.add_call_edge("caller_1", "target");

    let rev_deps = graph.compute_reverse_dependencies("target", 10);
    // target should NOT be included in reverse dependencies of itself
    assert_eq!(rev_deps.len(), 1);
    assert_eq!(rev_deps[0].symbol_id, "caller_1");
    assert_eq!(rev_deps[0].min_depth, 1);
}

#[test]
fn test_adversarial_two_node_mutual_recursion() {
    let mut graph = CallGraph::new();
    // Mutual recursion: A calls B and B calls A; A calls target
    graph.add_call_edge("node_A", "target");
    graph.add_call_edge("node_B", "node_A");
    graph.add_call_edge("node_A", "node_B");

    let rev_deps = graph.compute_reverse_dependencies("target", 10);
    assert_eq!(rev_deps.len(), 2);
    // node_A is at depth 1, node_B is at depth 2
    assert_eq!(rev_deps[0].symbol_id, "node_A");
    assert_eq!(rev_deps[0].min_depth, 1);
    assert!(rev_deps[0].is_direct);

    assert_eq!(rev_deps[1].symbol_id, "node_B");
    assert_eq!(rev_deps[1].min_depth, 2);
    assert!(!rev_deps[1].is_direct);
}

#[test]
fn test_adversarial_figure_eight_cycle() {
    let mut graph = CallGraph::new();
    // Cycle 1: target <- J <- A <- B <- J
    // Cycle 2: J <- C <- D <- J
    // J is junction node calling target
    graph.add_call_edge("J", "target");

    // Loop 1
    graph.add_call_edge("A", "J");
    graph.add_call_edge("B", "A");
    graph.add_call_edge("J", "B");

    // Loop 2
    graph.add_call_edge("C", "J");
    graph.add_call_edge("D", "C");
    graph.add_call_edge("J", "D");

    let rev_deps = graph.compute_reverse_dependencies("target", 10);
    // Discovered nodes should be: J (depth 1), A (depth 2), C (depth 2), B (depth 3), D (depth 3)
    assert_eq!(rev_deps.len(), 5);

    let j_dep = rev_deps.iter().find(|d| d.symbol_id == "J").unwrap();
    assert_eq!(j_dep.min_depth, 1);
    assert!(j_dep.is_direct);

    let a_dep = rev_deps.iter().find(|d| d.symbol_id == "A").unwrap();
    assert_eq!(a_dep.min_depth, 2);

    let c_dep = rev_deps.iter().find(|d| d.symbol_id == "C").unwrap();
    assert_eq!(c_dep.min_depth, 2);
}

#[test]
fn test_adversarial_clique_complete_graph_cycle() {
    let mut graph = CallGraph::new();
    // Complete graph K_6: all 6 nodes call each other
    let nodes = vec!["k0", "k1", "k2", "k3", "k4", "k5"];
    for &src in &nodes {
        for &dst in &nodes {
            graph.add_call_edge(src, dst);
        }
    }

    // k0 calls target
    graph.add_call_edge("k0", "target");

    let rev_deps = graph.compute_reverse_dependencies("target", 10);
    // k0 is direct (depth 1), all other 5 nodes are reached at depth 2 via k0 -> dst
    assert_eq!(rev_deps.len(), 6);
    assert_eq!(rev_deps[0].symbol_id, "k0");
    assert_eq!(rev_deps[0].min_depth, 1);

    for dep in &rev_deps[1..] {
        assert_eq!(dep.min_depth, 2);
        assert!(!dep.is_direct);
    }
}

#[test]
fn test_adversarial_deep_chain_exceeding_max_depth() {
    let mut graph = CallGraph::new();
    // Linear chain of 15 nodes: N15 -> N14 -> ... -> N1 -> target
    graph.add_call_edge("node_01", "target");
    for i in 2..=15 {
        let caller = format!("node_{:02}", i);
        let callee = format!("node_{:02}", i - 1);
        graph.add_call_edge(&caller, &callee);
    }

    // Traversal capped at max_depth = 10
    let rev_deps = graph.compute_reverse_dependencies("target", 10);
    // Only depths 1..=10 must be collected
    assert_eq!(rev_deps.len(), 10);
    assert_eq!(rev_deps.last().unwrap().min_depth, 10);
    assert_eq!(rev_deps.last().unwrap().symbol_id, "node_10");
}

// =========================================================================
// 2. THRESHOLD ESCALATION EMPIRICAL PROBING (10 vs 11 callers, risk scores)
// =========================================================================

#[test]
fn test_threshold_escalation_boundary_exact_ten() {
    let mut engine = KnowledgeGraphEngine::new();
    let target = "pkg::safe_fn";
    engine.register_symbol(create_test_symbol(target, "function"), true);

    // Register exactly 9 callers -> total affected = 1 (target) + 9 = 10 (<= 10 limit)
    for i in 1..=9 {
        let caller = format!("caller_{}", i);
        engine.register_symbol(create_test_symbol(&caller, "function"), true);
        engine.register_call(&caller, target);
    }

    let report = engine.compute_blast_radius(target).unwrap();
    assert_eq!(report.affected_symbols.len(), 10);
    // Exactly 10 with test coverage -> SAFE, no escalation
    assert!(report.is_safe());
    assert!(!report.requires_escalation());
}

#[test]
fn test_threshold_escalation_boundary_eleven_triggers_escalation() {
    let mut engine = KnowledgeGraphEngine::new();
    let target = "pkg::borderline_fn";
    engine.register_symbol(create_test_symbol(target, "function"), true);

    // Register exactly 10 callers -> total affected = 1 (target) + 10 = 11 (> 10 limit)
    for i in 1..=10 {
        let caller = format!("caller_{}", i);
        engine.register_symbol(create_test_symbol(&caller, "function"), true);
        engine.register_call(&caller, target);
    }

    let report = engine.compute_blast_radius(target).unwrap();
    assert_eq!(report.affected_symbols.len(), 11);
    // Exceeds 10 -> MUST require escalation
    assert!(!report.is_safe());
    assert!(report.requires_escalation());
}

#[test]
fn test_risk_score_escalation_on_critical_trait_with_zero_coverage() {
    let mut engine = KnowledgeGraphEngine::new();
    let target = "pkg::core_trait";
    // "trait" kind adds 15.0 criticality penalty, 0% coverage -> high score
    engine.register_symbol(create_test_symbol(target, "trait"), false);

    // 6 callers with 0 test coverage
    for i in 1..=6 {
        let caller = format!("caller_{}", i);
        engine.register_symbol(create_test_symbol(&caller, "function"), false);
        engine.register_call(&caller, target);
    }

    let report = engine.compute_blast_radius(target).unwrap();
    // affected_count = 7 <= 10, BUT risk score >= 70 triggers escalation
    assert!(
        report.risk_score >= 70,
        "Risk score {} should be >= 70",
        report.risk_score
    );
    assert!(!report.is_safe());
    assert!(report.requires_escalation());
}

// =========================================================================
// 3. MCP JSON-RPC 2.0 PROTOCOL CONFORMANCE AND ERROR PAYLOADS
// =========================================================================

#[test]
fn test_mcp_conformance_invalid_json() {
    let engine = KnowledgeGraphEngine::default();
    let invalid_json = "{ invalid json content ]";
    let resp_str = handle_mcp_request(&engine, invalid_json);
    let resp: McpResponse = serde_json::from_str(&resp_str).unwrap();

    assert_eq!(resp.jsonrpc, "2.0");
    assert_eq!(resp.id, None);
    assert!(resp.result.is_none());
    assert!(resp.error.is_some());

    let err = resp.error.unwrap();
    assert_eq!(err.code, -32700); // Standard Parse error code
    assert!(err.message.contains("Parse error"));
}

#[test]
fn test_mcp_conformance_unknown_method() {
    let engine = KnowledgeGraphEngine::default();
    let req = json!({
        "jsonrpc": "2.0",
        "id": 42,
        "method": "system/shutdown"
    });
    let resp_str = handle_mcp_request(&engine, &req.to_string());
    let resp: McpResponse = serde_json::from_str(&resp_str).unwrap();

    assert_eq!(resp.jsonrpc, "2.0");
    assert_eq!(resp.id, Some(json!(42)));
    assert!(resp.result.is_none());

    let err = resp.error.unwrap();
    assert_eq!(err.code, -32601); // Standard Method not found code
    assert!(err.message.contains("Method not found"));
}

#[test]
fn test_mcp_conformance_unknown_tool() {
    let engine = KnowledgeGraphEngine::default();
    let req = json!({
        "jsonrpc": "2.0",
        "id": "req-999",
        "method": "tools/call",
        "params": {
            "name": "non_existent_tool",
            "arguments": {
                "symbol_id": "foo"
            }
        }
    });
    let resp_str = handle_mcp_request(&engine, &req.to_string());
    let resp: McpResponse = serde_json::from_str(&resp_str).unwrap();

    assert_eq!(resp.jsonrpc, "2.0");
    assert_eq!(resp.id, Some(json!("req-999")));
    assert!(resp.result.is_none());

    let err = resp.error.unwrap();
    assert_eq!(err.code, -32601);
    assert!(err.message.contains("Unknown tool"));
}

#[test]
fn test_mcp_conformance_missing_params() {
    let engine = KnowledgeGraphEngine::default();
    let req = json!({
        "jsonrpc": "2.0",
        "id": "missing-params",
        "method": "tools/call"
    });
    let resp_str = handle_mcp_request(&engine, &req.to_string());
    let resp: McpResponse = serde_json::from_str(&resp_str).unwrap();

    assert_eq!(resp.jsonrpc, "2.0");
    assert_eq!(resp.id, Some(json!("missing-params")));
    let err = resp.error.unwrap();
    assert_eq!(err.code, -32602);
    assert!(err.message.contains("missing parameters"));
}

#[test]
fn test_mcp_conformance_missing_or_whitespace_symbol_id() {
    let engine = KnowledgeGraphEngine::default();
    // Test empty whitespace symbol_id
    let req = json!({
        "jsonrpc": "2.0",
        "id": "ws-symbol",
        "method": "tools/call",
        "params": {
            "name": "ast-blast-radius",
            "arguments": {
                "symbol_id": "   "
            }
        }
    });
    let resp_str = handle_mcp_request(&engine, &req.to_string());
    let resp: McpResponse = serde_json::from_str(&resp_str).unwrap();

    assert_eq!(resp.jsonrpc, "2.0");
    assert_eq!(resp.id, Some(json!("ws-symbol")));
    let err = resp.error.unwrap();
    assert_eq!(err.code, -32602);
    assert!(err.message.contains("Missing required argument 'symbol_id'"));
}

#[test]
fn test_mcp_conformance_unknown_symbol_not_found() {
    let engine = KnowledgeGraphEngine::default();
    let req = json!({
        "jsonrpc": "2.0",
        "id": "unknown-sym",
        "method": "tools/call",
        "params": {
            "name": "ast-blast-radius",
            "arguments": {
                "symbol_id": "ghost::does_not_exist"
            }
        }
    });
    let resp_str = handle_mcp_request(&engine, &req.to_string());
    let resp: McpResponse = serde_json::from_str(&resp_str).unwrap();

    assert_eq!(resp.jsonrpc, "2.0");
    assert_eq!(resp.id, Some(json!("unknown-sym")));
    let err = resp.error.unwrap();
    assert_eq!(err.code, -32004);
    assert!(err.message.contains("not found in knowledge graph index"));
}

#[test]
fn test_mcp_conformance_tools_list_schema_validity() {
    let engine = KnowledgeGraphEngine::default();
    let req = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list"
    });
    let resp_str = handle_mcp_request(&engine, &req.to_string());
    let resp: McpResponse = serde_json::from_str(&resp_str).unwrap();

    assert_eq!(resp.jsonrpc, "2.0");
    assert_eq!(resp.id, Some(json!(1)));
    assert!(resp.is_success());

    let res = resp.result.unwrap();
    let tools = res.tools.unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "ast-blast-radius");

    let schema = &tools[0].input_schema;
    assert_eq!(schema["type"], "object");
    assert!(schema["properties"]["symbol_id"].is_object());
    assert_eq!(schema["required"], json!(["symbol_id"]));
}

#[test]
fn test_mcp_conformance_full_report_fields() {
    let engine = KnowledgeGraphEngine::default();
    let target = "aura_core::hypervisor::wasm::execute_component";
    let req = json!({
        "jsonrpc": "2.0",
        "id": "full-test-01",
        "method": "tools/call",
        "params": {
            "name": "ast-blast-radius",
            "arguments": {
                "symbol_id": target
            }
        }
    });

    let resp_str = handle_mcp_request(&engine, &req.to_string());
    let resp: McpResponse = serde_json::from_str(&resp_str).unwrap();

    assert!(resp.is_success());
    let report = resp.parse_report().unwrap();

    // Verify all required fields from DISPATCH and PROJECT.md
    assert_eq!(report.target_symbol_id, target);
    assert_eq!(report.affected_count, report.affected_symbols.len());
    assert!(report.affected_symbols.contains(&target.to_string()));
    assert!(!report.transitively_affected_symbols.contains(&target.to_string()));
    assert_eq!(
        report.transitively_affected_symbols.len(),
        report.affected_count - 1
    );
    assert!(report.risk_score <= 100);
    assert!(report.test_coverage >= 0.0 && report.test_coverage <= 1.0);
    assert_eq!(report.requires_escalation, !report.is_safe);
}
