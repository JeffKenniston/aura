use rusqlite::{params, Connection, Result};
use std::path::Path;

pub struct KnowledgeGraph {
    conn: Connection,
}

#[derive(Debug, PartialEq)]
pub struct CallerDepth {
    pub caller_id: String,
    pub depth: u32,
}

impl KnowledgeGraph {
    /// Connects to the local SQLite knowledge graph. 
    /// If the database does not exist, it initializes the core schema.
    pub fn new<P: AsRef<Path>>(db_path: P) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        
        // Initialize the base static analysis schema
        conn.execute(
            "CREATE TABLE IF NOT EXISTS symbol_references (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                caller_id TEXT NOT NULL,
                callee_id TEXT NOT NULL,
                UNIQUE(caller_id, callee_id)
            )",
            [],
        )?;

        Ok(Self { conn })
    }

    /// Evaluates the blast radius of a targeted symbol using a recursive CTE.
    pub fn calculate_blast_radius(&self, target_symbol_id: &str) -> Result<Vec<CallerDepth>> {
        let mut stmt = self.conn.prepare(
            "
            WITH RECURSIVE CallChain AS (
                SELECT caller_id, callee_id, 1 AS depth
                FROM symbol_references
                WHERE callee_id = ?1
                
                UNION ALL
                
                SELECT sr.caller_id, sr.callee_id, cc.depth + 1
                FROM symbol_references sr
                JOIN CallChain cc ON sr.callee_id = cc.caller_id
                WHERE cc.depth < 10
            )
            SELECT caller_id, MIN(depth) as depth FROM CallChain GROUP BY caller_id;
            "
        )?;

        let caller_iter = stmt.query_map(params![target_symbol_id], |row| {
            Ok(CallerDepth {
                caller_id: row.get(0)?,
                depth: row.get(1)?,
            })
        })?;

        let mut results = Vec::new();
        for caller in caller_iter {
            results.push(caller?);
        }

        Ok(results)
    }

    /// Helper for inserting mock data during testing
    #[cfg(test)]
    pub fn insert_reference(&self, caller_id: &str, callee_id: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO symbol_references (caller_id, callee_id) VALUES (?1, ?2) ON CONFLICT DO NOTHING",
            params![caller_id, callee_id],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ast_blast_radius_cte() -> Result<()> {
        // Use an in-memory SQLite database for testing the CTE logic
        let kg = KnowledgeGraph::new(":memory:")?;

        // Construct a dependency graph:
        // A calls B
        // B calls Target
        // C calls B
        // D calls Target
        // E calls A
        
        kg.insert_reference("D", "Target")?;
        kg.insert_reference("B", "Target")?;
        kg.insert_reference("A", "B")?;
        kg.insert_reference("C", "B")?;
        kg.insert_reference("E", "A")?;

        let callers = kg.calculate_blast_radius("Target")?;
        
        // Target is called directly by D (depth 1), B (depth 1)
        // B is called by A (depth 2), C (depth 2)
        // A is called by E (depth 3)
        // Expected unique callers: B, D, A, C, E

        assert_eq!(callers.len(), 5);

        // Map depths for easy assertions
        let depth_map: std::collections::HashMap<_, _> = callers
            .into_iter()
            .map(|c| (c.caller_id, c.depth))
            .collect();

        assert_eq!(depth_map.get("D"), Some(&1));
        assert_eq!(depth_map.get("B"), Some(&1));
        assert_eq!(depth_map.get("A"), Some(&2));
        assert_eq!(depth_map.get("C"), Some(&2));
        assert_eq!(depth_map.get("E"), Some(&3));

        Ok(())
    }
}
