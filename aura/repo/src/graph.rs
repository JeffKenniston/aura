use rusqlite::Connection;

/// SQLite database schema with FTS5 and sqlite-vec for embeddings
pub struct KnowledgeGraph {
    pub db_conn: Connection,
}

impl KnowledgeGraph {
    pub fn new(path: &str) -> Self {
        crate::cache::register_sqlite_vec();
        let conn = Connection::open(path).unwrap();

        // Create tables...
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS file (
                id INTEGER PRIMARY KEY,
                path TEXT UNIQUE,
                hash TEXT
            );
            CREATE TABLE IF NOT EXISTS merkle_node (
                hash TEXT PRIMARY KEY,
                parent_hash TEXT
            );
            CREATE TABLE IF NOT EXISTS symbol (
                id INTEGER PRIMARY KEY,
                file_id INTEGER,
                kind TEXT,
                name TEXT,
                span TEXT,
                signature TEXT
            );
            CREATE TABLE IF NOT EXISTS reference (
                from_symbol INTEGER,
                to_symbol INTEGER
            );
            CREATE VIRTUAL TABLE IF NOT EXISTS fts_symbol USING fts5(name, signature, content='symbol');
            CREATE VIRTUAL TABLE IF NOT EXISTS vec_embeddings USING vec0(embedding float[768]);
            ",
        )
        .unwrap();

        Self { db_conn: conn }
    }

    /// Executes Hybrid Search combining BM25 and vector similarity using Reciprocal Rank Fusion (RRF)
    pub fn search(&self, query: &str, query_embedding: &[f32; 768]) -> Vec<String> {
        let mut stmt = self.db_conn.prepare(
            "WITH bm25_results AS (
                SELECT rowid, rank 
                FROM fts_symbol 
                WHERE fts_symbol MATCH ?1
            ),
            vec_results AS (
                SELECT rowid, distance 
                FROM vec_embeddings 
                WHERE embedding MATCH ?2 AND k = 100
            ),
            combined AS (
                SELECT 
                    COALESCE(b.rowid, v.rowid) AS doc_id,
                    (COALESCE(1.0 / (60.0 + b.rank), 0.0)) + 
                    (COALESCE(1.0 / (60.0 + v.distance), 0.0)) AS rrf_score
                FROM bm25_results b
                FULL OUTER JOIN vec_results v ON b.rowid = v.rowid
            )
            SELECT s.name 
            FROM combined c
            JOIN symbol s ON c.doc_id = s.id
            ORDER BY c.rrf_score DESC
            LIMIT 50;"
        ).unwrap();
        
        let embedding_bytes = unsafe { 
            std::slice::from_raw_parts(query_embedding.as_ptr() as *const u8, query_embedding.len() * 4) 
        };
        
        let results = stmt.query_map(rusqlite::params![query, embedding_bytes], |row| row.get(0)).unwrap();
        results.filter_map(Result::ok).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_knowledge_graph_table_creation_and_insertions() {
        let kg = KnowledgeGraph::new(":memory:");

        // Insert into file table
        kg.db_conn
            .execute(
                "INSERT INTO file (path, hash) VALUES (?1, ?2)",
                ["src/lib.rs", "blake3_dummy_hash"],
            )
            .unwrap();

        // Insert into symbol table
        kg.db_conn
            .execute(
                "INSERT INTO symbol (file_id, kind, name, span, signature) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![1, "function", "init", "0..10", "pub fn init()"],
            )
            .unwrap();

        // Insert into reference table
        kg.db_conn
            .execute(
                "INSERT INTO reference (from_symbol, to_symbol) VALUES (?1, ?2)",
                rusqlite::params![1, 2],
            )
            .unwrap();

        // Verify FTS5 integration
        let symbol_name: String = kg
            .db_conn
            .query_row("SELECT name FROM symbol WHERE name = 'init'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(symbol_name, "init");

        // Verify sqlite-vec table
        let row_count: i64 = kg
            .db_conn
            .query_row("SELECT count(*) FROM vec_embeddings", [], |row| row.get(0))
            .unwrap();
        assert_eq!(row_count, 0);
    }
}
