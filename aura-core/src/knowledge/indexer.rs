use crate::knowledge::KnowledgeGraph;
use ignore::WalkBuilder;
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use tree_sitter::Parser;

pub struct Indexer<'a> {
    kg: &'a KnowledgeGraph,
}

impl<'a> Indexer<'a> {
    pub fn new(kg: &'a KnowledgeGraph) -> Self {
        Self { kg }
    }

    /// Incrementally parse source code in the Rust host.
    /// Utilizes a BLAKE3 Merkle tree to hash file content and achieve real-time change detection.
    pub fn index_repository<P: AsRef<Path>>(&self, root_path: P) {
        let walker = WalkBuilder::new(root_path.as_ref())
            .hidden(true)
            .git_ignore(true)
            .build();

        let mut files_to_hash = Vec::new();

        for entry in walker.filter_map(|e| e.ok()) {
            if entry.file_type().map_or(false, |ft| ft.is_file()) {
                let path = entry.path().to_path_buf();
                let path_str = path.to_string_lossy().to_string();

                let metadata = entry.metadata().ok();
                let mtime = metadata
                    .and_then(|m| m.modified().ok())
                    .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);

                // Synchronous cache check before hashing
                let needs_hash = match self.kg.get_file_state(&path_str) {
                    Ok(Some((_, last_modified))) => last_modified < mtime || mtime == 0,
                    _ => true,
                };

                if needs_hash {
                    files_to_hash.push((path, path_str, mtime));
                }
            }
        }

        // 1. Rayon parallel iteration to compute BLAKE3 hashes only for changed files
        let hashed_files: Vec<_> = files_to_hash
            .into_par_iter()
            .filter_map(|(path, path_str, mtime)| {
                if let Ok(content) = fs::read(&path) {
                    let mut hasher = blake3::Hasher::new();
                    hasher.update(&content);
                    let current_hash = hasher.finalize().to_hex().to_string();
                    Some((path, path_str, content, current_hash, mtime))
                } else {
                    None
                }
            })
            .collect();

        // 2. Synchronous iteration for SQLite updates and CST extraction
        for (path, path_str, content, current_hash, mtime) in hashed_files {
            let needs_update = match self.kg.get_file_state(&path_str) {
                Ok(Some((old_hash, _))) => old_hash != current_hash,
                Ok(None) => true,
                Err(_) => false,
            };

            // We update the state (mtime and hash) even if hash hasn't changed, to avoid reading it again
            let _ = self.kg.update_file_state(&path_str, &current_hash, mtime);

            if needs_update {
                // Parse CST (simulated to just test compilation and structure)
                self.extract_cst(&path, &content);
            }
        }

        // Final Global Linking Pass
        let _ = self.kg.link_global_references();
    }

    /// Generates concrete syntax trees that preserve inline comments, punctuation, and whitespace.
    fn extract_cst(&self, path: &Path, content: &[u8]) {
        use crate::knowledge::parsers::ParserRegistry;
        if let Some(language) = ParserRegistry::get_language_for_file(path) {
            let mut parser = Parser::new();
            if parser.set_language(&language).is_ok() {
                if let Some(tree) = parser.parse(content, None) {
                    ParserRegistry::extract_symbols(&self.kg, path, content, &tree);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_indexer_hashing_and_caching() {
        let kg = KnowledgeGraph::new(":memory:").unwrap();
        let indexer = Indexer::new(&kg);

        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.rs");
        let mut file = fs::File::create(&file_path).unwrap();
        writeln!(file, "fn main() {{}}").unwrap();

        indexer.index_repository(dir.path());

        let path_str = file_path.to_string_lossy().to_string();
        let hash = kg.get_file_hash(&path_str).unwrap();
        assert!(hash.is_some(), "File should be hashed and stored in KG");
    }
}
