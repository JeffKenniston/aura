use serde::{Deserialize, Serialize};

/// Detailed symbol information corresponding to aura:graph WIT definition (TOOL-BR-001).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolInfo {
    pub id: String,
    pub file_id: String,
    pub kind: String,
    pub name: String,
    pub qualified_name: String,
    pub signature: String,
}

/// Risk evaluation report for an AST modification (TOOL-BR-003, TOOL-BR-004, TOOL-BR-005).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RiskReport {
    pub affected_symbols: Vec<String>,
    pub risk_score: u32,
    pub test_coverage: f32,
}

impl RiskReport {
    /// Blast radius threshold gating invariant: <= 10 affected symbols and risk score < 70 (TOOL-BR-005).
    pub const BLAST_RADIUS_THRESHOLD: usize = 10;
    pub const MAX_SAFE_RISK_SCORE: u32 = 70;

    /// Evaluates if the modification is safe to proceed autonomously without escalation.
    pub fn is_safe(&self) -> bool {
        self.affected_symbols.len() <= Self::BLAST_RADIUS_THRESHOLD
            && self.risk_score < Self::MAX_SAFE_RISK_SCORE
    }

    /// Evaluates if the modification exceeds threshold limits and requires human escalation (TOOL-BR-005).
    pub fn requires_escalation(&self) -> bool {
        !self.is_safe()
    }
}
