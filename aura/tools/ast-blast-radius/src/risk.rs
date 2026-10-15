use std::collections::HashMap;

use crate::cte::ReverseDependency;
use crate::types::RiskReport;

/// Computes the comprehensive risk score and test coverage for affected AST symbols
/// (TOOL-BR-003, TOOL-BR-004, TOOL-BR-005).
pub fn calculate_risk_report(
    target_symbol_id: &str,
    target_kind: &str,
    reverse_deps: &[ReverseDependency],
    coverage_map: &HashMap<String, bool>,
) -> RiskReport {
    let mut affected_symbols = Vec::with_capacity(reverse_deps.len() + 1);
    affected_symbols.push(target_symbol_id.to_string());
    for dep in reverse_deps {
        affected_symbols.push(dep.symbol_id.clone());
    }

    // 1. Calculate test coverage ratio across affected symbols (TOOL-BR-004)
    let covered_count = affected_symbols
        .iter()
        .filter(|sym| coverage_map.get(*sym).copied().unwrap_or(false))
        .count();

    let test_coverage = if affected_symbols.is_empty() {
        1.0
    } else {
        covered_count as f32 / affected_symbols.len() as f32
    };

    // 2. Compute risk score factors (TOOL-BR-003)
    let affected_count = affected_symbols.len();
    let max_depth = reverse_deps.iter().map(|d| d.min_depth).max().unwrap_or(0);
    let direct_callers_count = reverse_deps.iter().filter(|d| d.is_direct).count();

    let mut raw_score: f32 = 0.0;

    // Symbol volume factor
    raw_score += (affected_count as f32) * 5.5;

    // Call chain depth factor
    raw_score += (max_depth as f32) * 7.0;

    // Direct caller density factor
    raw_score += (direct_callers_count as f32) * 3.5;

    // Criticality multiplier for public interfaces/traits/core types
    if matches!(target_kind, "trait" | "interface" | "core" | "public_api") {
        raw_score += 15.0;
    }

    // Blast radius threshold gating penalty: > 10 downstream dependencies triggers escalation (TOOL-BR-005)
    if affected_count > RiskReport::BLAST_RADIUS_THRESHOLD {
        raw_score += 35.0;
    }

    // Test coverage mitigation: high test coverage discounts overall risk up to 45%
    let mitigated_score = raw_score * (1.0 - (test_coverage * 0.45));

    // Clamp score within [0, 100]
    let risk_score = (mitigated_score.round() as u32).min(100);

    RiskReport {
        affected_symbols,
        risk_score,
        test_coverage,
    }
}
