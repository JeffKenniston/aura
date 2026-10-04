pub mod indexer;
pub mod parsers;

use rusqlite::{params, Connection, Result};
use std::path::Path;

pub struct KnowledgeGraph {
    pub conn: Connection,
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

        // New tables for semantic repository intelligence
        conn.execute(
            "CREATE TABLE IF NOT EXISTS file_hashes (
                file_path TEXT PRIMARY KEY,
                blake3_hash TEXT NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS local_definitions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_path TEXT NOT NULL,
                symbol_name TEXT NOT NULL,
                symbol_type TEXT NOT NULL,
                start_byte INTEGER,
                end_byte INTEGER,
                UNIQUE(file_path, symbol_name)
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS symbol_usages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_path TEXT NOT NULL,
                reference_name TEXT NOT NULL,
                start_byte INTEGER,
                end_byte INTEGER
            )",
            [],
        )?;

        Ok(Self { conn })
    }

    /// Three-Pass Resolution:
    /// Pass 1: Local Definition Pass (Identify and store definitions)
    pub fn insert_local_definition(&self, file_path: &str, symbol_name: &str, symbol_type: &str, start_byte: usize, end_byte: usize) -> Result<()> {
        self.conn.execute(
            "INSERT INTO local_definitions (file_path, symbol_name, symbol_type, start_byte, end_byte) 
             VALUES (?1, ?2, ?3, ?4, ?5) 
             ON CONFLICT DO UPDATE SET 
             symbol_type = excluded.symbol_type, 
             start_byte = excluded.start_byte, 
             end_byte = excluded.end_byte",
            params![file_path, symbol_name, symbol_type, start_byte as i64, end_byte as i64],
        )?;
        Ok(())
    }

    /// Pass 2: Reference Extraction Pass (Extract imports and bare name invocations)
    pub fn insert_reference_usage(&self, file_path: &str, reference_name: &str, start_byte: usize, end_byte: usize) -> Result<()> {
        self.conn.execute(
            "INSERT INTO symbol_usages (file_path, reference_name, start_byte, end_byte) 
             VALUES (?1, ?2, ?3, ?4)",
            params![file_path, reference_name, start_byte as i64, end_byte as i64],
        )?;
        Ok(())
    }

    /// Pass 3: Global Linking Pass (Reconcile cross-file references)
    pub fn link_global_references(&self) -> Result<usize> {
        // Find references that match a known local definition
        let mut stmt = self.conn.prepare(
            "SELECT r.file_path, r.reference_name, ld.file_path, ld.symbol_name 
             FROM symbol_usages r 
             JOIN local_definitions ld ON r.reference_name = ld.symbol_name 
             WHERE r.file_path != ld.file_path"
        )?;

        let links_iter = stmt.query_map([], |row| {
            let caller_file: String = row.get(0)?;
            let reference_name: String = row.get(1)?;
            let callee_file: String = row.get(2)?;
            let symbol_name: String = row.get(3)?;
            Ok((format!("{}::{}", caller_file, reference_name), format!("{}::{}", callee_file, symbol_name)))
        })?;

        let mut linked_count = 0;
        for link in links_iter {
            if let Ok((caller, callee)) = link {
                // Insert into symbol_references
                self.conn.execute(
                    "INSERT INTO symbol_references (caller_id, callee_id) VALUES (?1, ?2) ON CONFLICT DO NOTHING",
                    params![caller, callee],
                )?;
                linked_count += 1;
            }
        }
        Ok(linked_count)
    }

    pub fn update_file_hash(&self, file_path: &str, blake3_hash: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO file_hashes (file_path, blake3_hash) 
             VALUES (?1, ?2) 
             ON CONFLICT(file_path) DO UPDATE SET blake3_hash = excluded.blake3_hash",
            params![file_path, blake3_hash],
        )?;
        Ok(())
    }

    pub fn get_file_hash(&self, file_path: &str) -> Result<Option<String>> {
        let mut stmt = self.conn.prepare("SELECT blake3_hash FROM file_hashes WHERE file_path = ?1")?;
        let mut rows = stmt.query(params![file_path])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
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
            ",
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
        let kg = KnowledgeGraph::new(":memory:")?;
        kg.insert_reference("D", "Target")?;
        kg.insert_reference("B", "Target")?;
        kg.insert_reference("A", "B")?;
        kg.insert_reference("C", "B")?;
        kg.insert_reference("E", "A")?;

        let callers = kg.calculate_blast_radius("Target")?;
        assert_eq!(callers.len(), 5);
        Ok(())
    }

    #[test]
    fn test_three_pass_resolution() -> Result<()> {
        let kg = KnowledgeGraph::new(":memory:")?;
        
        // Pass 1: Local definitions
        kg.insert_local_definition("core.rs", "Engine", "struct", 10, 50)?;
        kg.insert_local_definition("utils.rs", "helper", "function", 10, 50)?;

        // Pass 2: References
        kg.insert_reference_usage("main.rs", "Engine", 100, 110)?;
        kg.insert_reference_usage("main.rs", "helper", 150, 160)?;

        // Pass 3: Global Linking
        let linked = kg.link_global_references()?;
        assert_eq!(linked, 2);

        // Verify blast radius
        let callers = kg.calculate_blast_radius("core.rs::Engine")?;
        assert_eq!(callers.len(), 1);
        assert_eq!(callers[0].caller_id, "main.rs::Engine");
        
        Ok(())
    }
}
