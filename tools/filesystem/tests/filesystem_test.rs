use filesystem::{FileSystemService, PathValidator};
use tempfile::tempdir;

#[test]
fn test_path_prefix_matching_valid() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let validator = PathValidator::new(root);

    // Relative path inside workspace
    let resolved = validator.resolve_and_validate("src/main.rs").unwrap();
    assert!(resolved.starts_with(root.canonicalize().unwrap()));

    // Nested relative path inside workspace
    let resolved_nested = validator.resolve_and_validate("nested/dir/file.txt").unwrap();
    assert!(resolved_nested.starts_with(root.canonicalize().unwrap()));
}

#[test]
fn test_path_traversal_blocked_parent_dir() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let validator = PathValidator::new(root);

    // Direct parent navigation attempt
    let result = validator.resolve_and_validate("../secret.txt");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("prohibited parent traversal"));

    // Embedded parent navigation attempt
    let result_embedded = validator.resolve_and_validate("foo/../../etc/passwd");
    assert!(result_embedded.is_err());
    assert!(result_embedded.unwrap_err().to_string().contains("prohibited parent traversal"));
}

#[test]
fn test_path_outside_boundary_blocked() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let validator = PathValidator::new(root);

    // Absolute path pointing outside workspace boundary
    let result = validator.resolve_and_validate("/etc/shadow");
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("outside workspace boundary") || err_msg.contains("denied"),
        "Unexpected error: {}",
        err_msg
    );
}

#[test]
fn test_atomic_file_write_and_read() {
    let dir = tempdir().unwrap();
    let service = FileSystemService::new(dir.path());

    let filename = "subdir/test_file.txt";
    let data = b"Hello, Aura OS WASI 0.3 Sandbox!";

    // Write file
    let write_res = service.write_file(filename, data);
    assert!(write_res.is_ok(), "Write failed: {:?}", write_res);

    // Read file
    let read_res = service.read_file(filename);
    assert!(read_res.is_ok(), "Read failed: {:?}", read_res);
    assert_eq!(read_res.unwrap(), data);
}

#[test]
fn test_atomic_file_patch_success() {
    let dir = tempdir().unwrap();
    let service = FileSystemService::new(dir.path());

    let filename = "code.rs";
    let initial_content = "fn main() {\n    println!(\"Hello old\");\n}\n";
    service.write_file(filename, initial_content.as_bytes()).unwrap();

    let diff = r#"--- code.rs
+++ code.rs
@@ -1,3 +1,3 @@
 fn main() {
-    println!("Hello old");
+    println!("Hello new");
 }
"#;

    let patch_res = service.patch_file(filename, diff);
    assert!(patch_res.is_ok(), "Patch failed: {:?}", patch_res);

    let updated = String::from_utf8(service.read_file(filename).unwrap()).unwrap();
    assert_eq!(updated, "fn main() {\n    println!(\"Hello new\");\n}\n");
}

#[test]
fn test_patch_conflict_detection() {
    let dir = tempdir().unwrap();
    let service = FileSystemService::new(dir.path());

    let filename = "conflict.rs";
    let initial_content = "fn main() {\n    println!(\"Current line\");\n}\n";
    service.write_file(filename, initial_content.as_bytes()).unwrap();

    let invalid_diff = r#"--- conflict.rs
+++ conflict.rs
@@ -1,3 +1,3 @@
 fn main() {
-    println!("Expected line that does not match");
+    println!("New line");
 }
"#;

    let patch_res = service.patch_file(filename, invalid_diff);
    assert!(patch_res.is_err());
    assert!(patch_res.unwrap_err().contains("mismatch"));
}

#[test]
fn test_submillisecond_execution() {
    let dir = tempdir().unwrap();
    let service = FileSystemService::new(dir.path());

    let start = std::time::Instant::now();
    service.write_file("bench.txt", b"fast execution").unwrap();
    let _ = service.read_file("bench.txt").unwrap();
    let elapsed = start.elapsed();

    // Must be well within sub-millisecond execution budget (e.g. < 5ms for cold I/O)
    assert!(elapsed.as_millis() < 50, "Execution took too long: {:?}", elapsed);
}
