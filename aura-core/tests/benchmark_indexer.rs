use aura_core::knowledge::{KnowledgeGraph, indexer::Indexer};
use std::fs;
use std::io::Write;
use tempfile::tempdir;
use std::time::Instant;

#[test]
fn benchmark_indexer() {
    let kg = KnowledgeGraph::new(":memory:").unwrap();
    let indexer = Indexer::new(&kg);

    let dir = tempdir().unwrap();
    // Create 1000 files
    for i in 0..1000 {
        let file_path = dir.path().join(format!("test_{}.rs", i));
        let mut file = fs::File::create(&file_path).unwrap();
        writeln!(file, "fn main() {{ println!(\"{}\"); }}", i).unwrap();
    }

    let start = Instant::now();
    indexer.index_repository(dir.path());
    let duration1 = start.elapsed();
    println!("First indexing (cold): {:?}", duration1);

    let start = Instant::now();
    indexer.index_repository(dir.path());
    let duration2 = start.elapsed();
    println!("Second indexing (warm): {:?}", duration2);

    assert!(duration2 < duration1);
}
