use rusqlite::Connection;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

pub struct WasmTools;

impl WasmTools {
    fn get_base_dir() -> String {
        std::env::var("AURA_BASE_DIR").unwrap_or_else(|_| "/home/jeff/aura".to_string())
    }

    pub fn fs_read(path_str: &str) -> Result<String, Box<dyn Error>> {
        let path = Path::new(path_str);
        let base_dir = Self::get_base_dir();
        if !path.starts_with(&base_dir) {
            return Err(format!("Path traversal blocked: Access restricted to {}", base_dir).into());
        }
        if path.components().any(|c| c.as_os_str() == "..") {
            return Err("Path traversal blocked: .. is not allowed".into());
        }
        Ok(fs::read_to_string(path)?)
    }

    pub fn fs_write(path_str: &str, content: &str) -> Result<(), Box<dyn Error>> {
        let path = Path::new(path_str);
        let base_dir = Self::get_base_dir();
        if !path.starts_with(&base_dir) {
            return Err(format!("Path traversal blocked: Access restricted to {}", base_dir).into());
        }
        if path.components().any(|c| c.as_os_str() == "..") {
            return Err("Path traversal blocked: .. is not allowed".into());
        }
        fs::write(path, content)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::env;
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_wasm_fs_write_success() {
        let dir = tempdir().unwrap();
        let base_path = dir.path().to_str().unwrap().to_string();
        env::set_var("AURA_BASE_DIR", &base_path);

        let file_path = format!("{}/test.txt", base_path);
        let content = "hello world";

        let result = WasmTools::fs_write(&file_path, content);
        assert!(result.is_ok());

        let read_content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(read_content, content);
    }

    #[test]
    #[serial]
    fn test_wasm_fs_write_blocked_outside_base() {
        let dir = tempdir().unwrap();
        let base_path = dir.path().to_str().unwrap().to_string();
        env::set_var("AURA_BASE_DIR", &base_path);

        let result = WasmTools::fs_write("/tmp/outside_test.txt", "content");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Path traversal blocked"));
    }

    #[test]
    #[serial]
    fn test_wasm_fs_write_blocked_traversal() {
        let dir = tempdir().unwrap();
        let base_path = dir.path().to_str().unwrap().to_string();
        env::set_var("AURA_BASE_DIR", &base_path);

        let file_path = format!("{}/../test.txt", base_path);
        let result = WasmTools::fs_write(&file_path, "content");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains(".. is not allowed"));
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
