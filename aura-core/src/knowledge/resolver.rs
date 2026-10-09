use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// An incremental three-pass cross-file import resolver.
pub struct Resolver {
    pub symbols: HashMap<PathBuf, HashSet<String>>,
    pub imports: HashMap<PathBuf, HashSet<String>>,
}

impl Resolver {
    pub fn new() -> Self {
        Self {
            symbols: HashMap::new(),
            imports: HashMap::new(),
        }
    }

    /// Pass 1: Extracts symbols from a file
    pub fn extract_symbols(&mut self, file: &std::path::Path, symbols: HashSet<String>) {
        self.symbols.insert(file.to_path_buf(), symbols);
    }

    /// Pass 2: Extracts imports from a file
    pub fn extract_imports(&mut self, file: &std::path::Path, imports: HashSet<String>) {
        self.imports.insert(file.to_path_buf(), imports);
    }

    /// Pass 3: Resolves imports across the project graph for changed files
    pub fn resolve(&self, changed_files: &[PathBuf]) -> HashMap<PathBuf, Vec<PathBuf>> {
        let mut resolutions = HashMap::new();
        for file in changed_files {
            let mut resolved_to = Vec::new();
            if let Some(file_imports) = self.imports.get(file) {
                for (other_file, other_symbols) in &self.symbols {
                    if file != other_file {
                        for imp in file_imports {
                            if other_symbols.contains(imp) {
                                resolved_to.push(other_file.clone());
                            }
                        }
                    }
                }
            }
            resolutions.insert(file.clone(), resolved_to);
        }
        resolutions
    }

    pub fn check_blast_radius(
        &self,
        db: &rusqlite::Connection,
        symbol_id: i64,
    ) -> Result<BlastRadiusReport, String> {
        let query = "
            WITH RECURSIVE downstream AS (
                SELECT to_symbol, 1 as depth FROM reference WHERE from_symbol = ?1
                UNION ALL
                SELECT r.to_symbol, d.depth + 1
                FROM reference r
                JOIN downstream d ON r.from_symbol = d.to_symbol
                WHERE d.depth < 10
            )
            SELECT COUNT(DISTINCT to_symbol) FROM downstream;
        ";

        let mut stmt = db.prepare(query).map_err(|e| e.to_string())?;
        let count: i64 = stmt
            .query_row([symbol_id], |row| row.get(0))
            .map_err(|e| e.to_string())?;

        let action = if count > 20 {
            Action::Suspend
        } else if count > 5 {
            Action::Escalate
        } else {
            Action::Proceed
        };

        Ok(BlastRadiusReport {
            impacted_count: count,
            action,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Proceed,
    Escalate,
    Suspend,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlastRadiusReport {
    pub impacted_count: i64,
    pub action: Action,
}

impl Default for Resolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::graph::KnowledgeGraph;
    use crate::transport::webhook::QueueSuspensionManager;

    #[test]
    fn test_three_pass_import_resolver() {
        let mut resolver = Resolver::new();
        let file_a = PathBuf::from("src/a.rs");
        let file_b = PathBuf::from("src/b.rs");

        let mut symbols_a = HashSet::new();
        symbols_a.insert("TypeA".to_string());
        resolver.extract_symbols(&file_a, symbols_a);

        let mut imports_b = HashSet::new();
        imports_b.insert("TypeA".to_string());
        resolver.extract_imports(&file_b, imports_b);

        let resolved = resolver.resolve(std::slice::from_ref(&file_b));
        assert_eq!(resolved.get(&file_b).unwrap(), &vec![file_a]);
    }

    #[test]
    fn test_blast_radius_thresholds() {
        let kg = KnowledgeGraph::new(":memory:");
        let resolver = Resolver::new();

        // 1. Test low impact (count <= 5) -> Proceed
        // Symbol 1 has 3 downstream references (symbols 2, 3, 4)
        for target in 2..=4 {
            kg.db_conn
                .execute(
                    "INSERT INTO reference (from_symbol, to_symbol) VALUES (?1, ?2)",
                    rusqlite::params![1, target],
                )
                .unwrap();
        }
        let report_low = resolver.check_blast_radius(&kg.db_conn, 1).unwrap();
        assert_eq!(report_low.impacted_count, 3);
        assert_eq!(report_low.action, Action::Proceed);

        // 2. Test medium impact (5 < count <= 20) -> Escalate
        // Symbol 10 has 8 downstream references (symbols 11..=18)
        for target in 11..=18 {
            kg.db_conn
                .execute(
                    "INSERT INTO reference (from_symbol, to_symbol) VALUES (?1, ?2)",
                    rusqlite::params![10, target],
                )
                .unwrap();
        }
        let report_med = resolver.check_blast_radius(&kg.db_conn, 10).unwrap();
        assert_eq!(report_med.impacted_count, 8);
        assert_eq!(report_med.action, Action::Escalate);
    }

    #[test]
    fn test_force_refactoring_widely_used_symbol_suspends_for_cryptographic_signature() {
        let kg = KnowledgeGraph::new(":memory:");
        let resolver = Resolver::new();

        // Widely used core symbol: ID 100
        // Directly or transitively referenced by 25 downstream symbols (> 20 threshold)
        for target in 101..=125 {
            kg.db_conn
                .execute(
                    "INSERT INTO reference (from_symbol, to_symbol) VALUES (?1, ?2)",
                    rusqlite::params![100, target],
                )
                .unwrap();
        }

        // Evaluate Blast Radius for refactoring symbol 100
        let report = resolver
            .check_blast_radius(&kg.db_conn, 100)
            .expect("Failed to query blast radius");

        assert_eq!(report.impacted_count, 25);
        assert_eq!(
            report.action,
            Action::Suspend,
            "Refactoring a widely used symbol (>20 references) must evaluate to Action::Suspend"
        );

        // Simulate execution flow responding to Action::Suspend:
        // 1. Task Queue is suspended
        let suspension_manager = QueueSuspensionManager::new(std::time::Duration::from_secs(5));
        let task_id = "task-refactor-core-symbol-100";
        suspension_manager.suspend_tenant(task_id);
        assert!(
            suspension_manager.is_suspended(task_id),
            "Task queue must be marked as suspended"
        );

        // 2. Cryptographic signature verification for administrative approval
        let admin_secret_key = [7u8; 32];
        let payload = format!(
            "APPROVE_REFACTOR:symbol_100:blast_radius_{}",
            report.impacted_count
        );

        // Function simulating cryptographic signature verification
        let verify_signature = |payload: &str, sig: &str, key: &[u8; 32]| -> bool {
            let expected_mac = blake3::keyed_hash(key, payload.as_bytes())
                .to_hex()
                .to_string();
            expected_mac == sig
        };

        // Case A: Invalid / Untrusted signature is rejected, task remains suspended
        let invalid_signature =
            "bad_signature_00000000000000000000000000000000000000000000000000000000";
        let is_valid = verify_signature(&payload, invalid_signature, &admin_secret_key);
        assert!(
            !is_valid,
            "Invalid cryptographic signature must be rejected"
        );
        assert!(
            suspension_manager.is_suspended(task_id),
            "Task must remain suspended when cryptographic signature is invalid"
        );

        // Case B: Valid cryptographic signature from authorized security admin approves resumption
        let valid_signature = blake3::keyed_hash(&admin_secret_key, payload.as_bytes())
            .to_hex()
            .to_string();
        let is_valid = verify_signature(&payload, &valid_signature, &admin_secret_key);
        assert!(is_valid, "Valid cryptographic signature must be accepted");

        // Upon valid cryptographic approval, resume the suspended task queue
        suspension_manager.resume_tenant(task_id);
        assert!(
            !suspension_manager.is_suspended(task_id),
            "Task queue must be resumed following cryptographic signature approval"
        );
    }
}
