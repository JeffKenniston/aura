use rusqlite::Connection;
use std::error::Error;
use std::fs;
use std::path::Path;

pub struct WasmTools;

impl WasmTools {
    pub fn fs_read(path_str: &str) -> Result<String, Box<dyn Error>> {
        let path = Path::new(path_str);
        let current_dir = std::env::current_dir()?;
        let allowed_prefix = current_dir.to_str().ok_or("Invalid UTF-8 in current directory path")?.to_string();
        if !path.starts_with(&allowed_prefix) {
            return Err(format!("Path traversal blocked: Access restricted to {}", allowed_prefix).into());
        }
        if path.components().any(|c| c.as_os_str() == "..") {
            return Err("Path traversal blocked: .. is not allowed".into());
        }
        Ok(fs::read_to_string(path)?)
    }

    pub fn fs_write(path_str: &str, content: &str) -> Result<(), Box<dyn Error>> {
        let path = Path::new(path_str);
        let current_dir = std::env::current_dir()?;
        let allowed_prefix = current_dir.to_str().ok_or("Invalid UTF-8 in current directory path")?.to_string();
        if !path.starts_with(&allowed_prefix) {
            return Err(format!("Path traversal blocked: Access restricted to {}", allowed_prefix).into());
        }
        if path.components().any(|c| c.as_os_str() == "..") {
            return Err("Path traversal blocked: .. is not allowed".into());
        }
        fs::write(path, content)?;
        Ok(())
    }

    pub fn analyze_blast_radius(target_symbol_id: &str) -> Result<u32, Box<dyn Error>> {
        let base_path = std::env::current_dir()?;
        let db_path = base_path.join(".agents/knowledge_graph.sqlite");
        if !db_path.exists() {
            return Ok(0); // If no DB, blast radius is 0
        }
        let conn = Connection::open(db_path)?;

        let query = r#"
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
            SELECT COUNT(DISTINCT caller_id) FROM CallChain;
        "#;

        let mut stmt = conn.prepare(query)?;
        let count: u32 = stmt
            .query_row([target_symbol_id], |row| row.get(0))
            .unwrap_or(0);

        Ok(count)
    }

    pub fn mint_ephemeral_svid(agent_name: &str) -> Result<String, Box<dyn Error>> {
        // Mock SPIRE Workload API interaction to mint an SVID
        // Real implementation would connect to /tmp/spire-agent/public/api.sock
        let svid = format!(
            "spiffe://aura.local/ephemeral/{}/{}",
            agent_name,
            uuid::Uuid::new_v4()
        );
        Ok(svid)
    }
}

pub fn bind_to_linker(
    linker: &mut wasmtime::component::Linker<crate::hypervisor::wasm::WasmState>,
) -> Result<(), Box<dyn Error>> {
    // In a full Component Model implementation with bindgen!, we would implement a trait.
    // For now, we simulate linking the host functions.
    // In Component Model, root func wraps are somewhat limited without WIT, but we demonstrate
    // the static composition structure here.

    // We would do linker.root().func_wrap("wasi:tools/filesystem", "read", |...|) here.

    Ok(())
}
