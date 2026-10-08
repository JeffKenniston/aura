use ast_blast_radius::{CallGraph, KnowledgeGraphEngine, SymbolInfo};

#[test]
fn test_hybrid_search_bm25_matching() {
    let engine = KnowledgeGraphEngine::default();

    // Query for "execute_component"
    let results = engine.hybrid_search("execute_component", 5).unwrap();
    assert!(!results.is_empty());
    assert_eq!(results[0].name, "execute_component");

    // Partial query
    let partial_results = engine.hybrid_search("enforcer", 5).unwrap();
    assert!(!partial_results.is_empty());
    assert!(partial_results[0].qualified_name.contains("CapabilityEnforcer"));
}

#[test]
fn test_recursive_cte_dependency_traversal() {
    let mut graph = CallGraph::new();

    // Setup linear chain: A -> B -> C -> D
    // (A calls B, B calls C, C calls D)
    graph.add_call_edge("node_A", "node_B");
    graph.add_call_edge("node_B", "node_C");
    graph.add_call_edge("node_C", "node_D");

    // Reverse dependencies of D should be C (depth 1), B (depth 2), A (depth 3)
    let rev_deps = graph.compute_reverse_dependencies("node_D", 10);
    assert_eq!(rev_deps.len(), 3);

    assert_eq!(rev_deps[0].symbol_id, "node_C");
    assert_eq!(rev_deps[0].min_depth, 1);
    assert!(rev_deps[0].is_direct);

    assert_eq!(rev_deps[1].symbol_id, "node_B");
    assert_eq!(rev_deps[1].min_depth, 2);
    assert!(!rev_deps[1].is_direct);

    assert_eq!(rev_deps[2].symbol_id, "node_A");
    assert_eq!(rev_deps[2].min_depth, 3);
    assert!(!rev_deps[2].is_direct);
}

#[test]
fn test_recursive_cte_cycle_safety() {
    let mut graph = CallGraph::new();

    // Cyclic graph: A calls B, B calls C, C calls A
    graph.add_call_edge("node_A", "node_B");
    graph.add_call_edge("node_B", "node_C");
    graph.add_call_edge("node_C", "node_A");

    // Must not loop indefinitely
    let rev_deps = graph.compute_reverse_dependencies("node_A", 10);
    assert_eq!(rev_deps.len(), 2);
}

#[test]
fn test_blast_radius_safe_refactor() {
    let engine = KnowledgeGraphEngine::default();

    // execute_component has 2 upstream callers (tests::integration::test_wasi_flow, aura_cli::commands::run::execute)
    // Total affected symbols: 3 <= 10 -> Safe!
    let report = engine
        .compute_blast_radius("aura_core::hypervisor::wasm::execute_component")
        .unwrap();

    assert_eq!(report.affected_symbols.len(), 3);
    assert!(report.is_safe());
    assert!(!report.requires_escalation());
    assert!(report.test_coverage > 0.0);
}

#[test]
fn test_blast_radius_threshold_escalation() {
    let mut engine = KnowledgeGraphEngine::new();

    let target_id = "core::critical_api";
    engine.register_symbol(
        SymbolInfo {
            id: target_id.to_string(),
            file_id: "core/src/api.rs".to_string(),
            kind: "trait".to_string(),
            name: "critical_api".to_string(),
            qualified_name: "core::critical_api".to_string(),
            signature: "pub trait CriticalApi".to_string(),
        },
        true,
    );

    // Register 12 distinct callers (> 10 threshold limit)
    for i in 1..=12 {
        let caller_id = format!("module_{}::caller", i);
        engine.register_symbol(
            SymbolInfo {
                id: caller_id.clone(),
                file_id: format!("module_{}/src/lib.rs", i),
                kind: "function".to_string(),
                name: "caller".to_string(),
                qualified_name: caller_id.clone(),
                signature: "fn caller()".to_string(),
            },
            false,
        );
        engine.register_call(&caller_id, target_id);
    }

    let report = engine.compute_blast_radius(target_id).unwrap();

    // Total affected = 13 (target + 12 callers) > 10
    assert_eq!(report.affected_symbols.len(), 13);
    assert!(report.requires_escalation(), "Expected blast radius escalation");
    assert!(!report.is_safe());
    assert!(report.risk_score >= 70);
}

#[test]
fn test_submillisecond_execution() {
    let engine = KnowledgeGraphEngine::default();

    let start = std::time::Instant::now();
    for _ in 0..100 {
        let _ = engine.hybrid_search("execute", 5).unwrap();
        let _ = engine
            .compute_blast_radius("aura_core::hypervisor::wasm::execute_component")
            .unwrap();
    }
    let elapsed = start.elapsed();

    // 100 iterations must take well under 10ms (sub-millisecond per query)
    assert!(elapsed.as_millis() < 10, "Execution took too long: {:?}", elapsed);
}
