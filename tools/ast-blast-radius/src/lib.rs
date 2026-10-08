pub mod cte;
pub mod graph;
pub mod risk;
pub mod types;

pub use cte::{CallGraph, ReverseDependency};
pub use graph::KnowledgeGraphEngine;
pub use risk::calculate_risk_report;
pub use types::{RiskReport, SymbolInfo};

// Generate WIT bindings for the aura:graph WebAssembly component
wit_bindgen::generate!({
    path: "wit",
    world: "blast-radius",
});

struct BlastRadiusComponent;

impl exports::aura::graph::query::Guest for BlastRadiusComponent {
    fn hybrid_search(
        query: String,
        limit: u32,
    ) -> Result<Vec<exports::aura::graph::query::SymbolInfo>, String> {
        let engine = KnowledgeGraphEngine::default();
        let results = engine.hybrid_search(&query, limit)?;
        let mapped = results
            .into_iter()
            .map(|s| exports::aura::graph::query::SymbolInfo {
                id: s.id,
                file_id: s.file_id,
                kind: s.kind,
                name: s.name,
                qualified_name: s.qualified_name,
                signature: s.signature,
            })
            .collect();
        Ok(mapped)
    }

    fn compute_blast_radius(
        symbol_id: String,
    ) -> Result<exports::aura::graph::query::RiskReport, String> {
        let engine = KnowledgeGraphEngine::default();
        let report = engine.compute_blast_radius(&symbol_id)?;
        Ok(exports::aura::graph::query::RiskReport {
            affected_symbols: report.affected_symbols,
            risk_score: report.risk_score,
            test_coverage: report.test_coverage,
        })
    }
}

#[cfg(target_arch = "wasm32")]
export!(BlastRadiusComponent);
