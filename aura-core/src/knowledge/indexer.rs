use rayon::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Computes BLAKE3 hash for file contents
pub fn hash_file(path: &Path) -> Result<String, std::io::Error> {
    let content = std::fs::read(path)?;
    let hash = blake3::hash(&content);
    Ok(hash.to_hex().to_string())
}

/// A dynamic Merkle tree using BLAKE3 to detect changed files.
pub struct MerkleTree {
    pub root_hash: String,
    pub file_hashes: HashMap<PathBuf, String>,
}

impl MerkleTree {
    pub fn build(files: &[PathBuf]) -> Self {
        let hashes: Vec<(PathBuf, String)> = files
            .par_iter()
            .filter_map(|p| hash_file(p).ok().map(|h| (p.clone(), h)))
            .collect();

        let mut file_hashes = HashMap::new();
        let mut combined_hash = blake3::Hasher::new();

        for (p, h) in hashes {
            combined_hash.update(h.as_bytes());
            file_hashes.insert(p, h);
        }

        Self {
            root_hash: combined_hash.finalize().to_hex().to_string(),
            file_hashes,
        }
    }

    pub fn update(&mut self, files: &[PathBuf]) -> Vec<PathBuf> {
        let hashes: Vec<(PathBuf, String)> = files
            .par_iter()
            .filter_map(|p| hash_file(p).ok().map(|h| (p.clone(), h)))
            .collect();

        let mut changed_files = Vec::new();
        let mut new_hashes = HashMap::new();
        let mut combined_hash = blake3::Hasher::new();

        for (p, h) in hashes {
            combined_hash.update(h.as_bytes());
            if self.file_hashes.get(&p) != Some(&h) {
                changed_files.push(p.clone());
            }
            new_hashes.insert(p, h);
        }

        self.root_hash = combined_hash.finalize().to_hex().to_string();
        self.file_hashes = new_hashes;

        changed_files
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::Instant;

    #[test]
    fn test_merkle_tree_generation_and_incremental_update() {
        let temp_dir = tempfile::tempdir().unwrap();
        let f1 = temp_dir.path().join("a.rs");
        let f2 = temp_dir.path().join("b.rs");

        std::fs::write(&f1, "fn a() {}").unwrap();
        std::fs::write(&f2, "fn b() {}").unwrap();

        let files = vec![f1.clone(), f2.clone()];
        let mut tree = MerkleTree::build(&files);
        let initial_root = tree.root_hash.clone();
        assert!(!initial_root.is_empty());
        assert_eq!(tree.file_hashes.len(), 2);

        // No modifications: update should return empty
        let changed = tree.update(&files);
        assert!(changed.is_empty());
        assert_eq!(tree.root_hash, initial_root);

        // Modify a single file
        std::fs::write(&f1, "fn a_modified() {}").unwrap();
        let changed = tree.update(&files);
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0], f1);
        assert_ne!(tree.root_hash, initial_root);
    }

    #[test]
    fn test_single_file_modification_reindex_under_50ms() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut files = Vec::new();

        // Create 50 Rust source files to simulate a substantial module
        for i in 0..50 {
            let file_path = temp_dir.path().join(format!("module_{}.rs", i));
            let content = format!(
                "pub struct Struct{} {{ pub val: usize }}\npub fn func{}() -> usize {{ {} }}\n",
                i, i, i
            );
            std::fs::write(&file_path, content).unwrap();
            files.push(file_path);
        }

        // Initial build
        let mut tree = MerkleTree::build(&files);

        // Modify a single Rust file
        let target_file = &files[10];
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(target_file)
            .unwrap();
        writeln!(f, "// modified line").unwrap();
        drop(f);

        // Measure re-index time (incremental hash detection + parse)
        let start = Instant::now();
        let changed_files = tree.update(&files);

        // Re-parse the changed file with tree-sitter
        for changed in &changed_files {
            let bytes = std::fs::read(changed).unwrap();
            let _cst =
                crate::knowledge::cst::parse(&bytes, "rs").expect("Failed to parse changed file");
        }
        let elapsed = start.elapsed();

        println!(
            "[PERF VERIFICATION] Re-index time for single file modification across {} files: {:?}",
            files.len(),
            elapsed
        );

        assert_eq!(changed_files.len(), 1);
        assert_eq!(&changed_files[0], target_file);
        assert!(
            elapsed < std::time::Duration::from_millis(50),
            "Re-index took {:?}, which exceeds the 50ms SLA!",
            elapsed
        );
    }
}
