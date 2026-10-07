use rusqlite::Connection;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

pub struct WasmTools;

impl WasmTools {
    pub fn fs_read(path_str: &str) -> Result<String, Box<dyn Error>> {
        let path = Path::new(path_str);

        // Use canonicalize to resolve symlinks and absolute paths
        let canonical_path = fs::canonicalize(path)?;

        if !canonical_path.starts_with("/home/jeff/aura") {
            return Err("Path traversal blocked: Access restricted to /home/jeff/aura".into());
        }

        Ok(fs::read_to_string(canonical_path)?)
    }

    pub fn fs_write(path_str: &str, content: &str) -> Result<(), Box<dyn Error>> {
        let path = Path::new(path_str);

        // To canonicalize a file for writing, the file might not exist yet.
        let parent = path.parent().unwrap_or_else(|| Path::new(""));
        let canonical_parent = fs::canonicalize(parent)?;

        if !canonical_parent.starts_with("/home/jeff/aura") {
            return Err("Path traversal blocked: Access restricted to /home/jeff/aura".into());
        }

        // We can create the path by joining the canonical parent and the file name
        let file_name = path.file_name().ok_or("Invalid file name")?;
        let canonical_path = canonical_parent.join(file_name);

        // Symlinks (both broken and existing) can point anywhere. If the path exists as a symlink,
        // we should either refuse to overwrite it or resolve it fully.
        if let Ok(metadata) = fs::symlink_metadata(&canonical_path) {
            if metadata.is_symlink() {
                // If it is a symlink, canonicalize it and check the actual path
                let fully_canonical = fs::canonicalize(&canonical_path)?;
                if !fully_canonical.starts_with("/home/jeff/aura") {
                    return Err("Path traversal blocked: Access restricted to /home/jeff/aura".into());
                }
            }
        }

        fs::write(canonical_path, content)?;
        Ok(())
    }

    pub fn analyze_blast_radius(target_symbol_id: &str) -> Result<u32, Box<dyn Error>> {
        let db_path = "/home/jeff/aura/.agents/knowledge_graph.sqlite";
        if !Path::new(db_path).exists() {
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
