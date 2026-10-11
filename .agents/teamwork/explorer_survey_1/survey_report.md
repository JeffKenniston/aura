# Survey Report: Git Drift & MerkleTree (M4 Repo Intelligence)

**Author:** Survey Explorer 1  
**Date:** 2026-10-09  
**Target Subsystem:** `aura-core/src/knowledge/git.rs` & `aura-core/src/knowledge/indexer.rs`  
**Referenced Constraints:** `aura-repo-domain`, `aura-architecture` (Invariants 01, 06)

---

## 1. Executive Summary

This investigation surveys the implementation status and integration mechanics of Git drift detection for milestone M4 (Repo Intelligence).

The investigation verified:
1. `git.rs` is located at `/home/jeff/aura/aura-core/src/knowledge/git.rs`. Currently, `GitManager` is a minimal 43-line stub with four placeholder functions (`check_drift`, `sandbox_lint`, `apply_patch`, `open_pr`). `check_drift` unconditionally returns `Ok(())` without inspecting file hashes or detecting conflicts.
2. `indexer.rs` is located at `/home/jeff/aura/aura-core/src/knowledge/indexer.rs`. It already implements `MerkleTree` with BLAKE3 hashing and Rayon parallelization (`MerkleTree::build` and `MerkleTree::update`). `update(&mut self, files: &[PathBuf]) -> Vec<PathBuf>` accurately recalculates BLAKE3 file digests and returns the list of files modified since the baseline tree was established in under 50ms (sub-millisecond benchmarks observed).
3. The test suite in `aura-core` currently passes 74 tests across configuration, bus, hypervisor, identity, cache, cst, indexer, resolver, runtime, supervisor, and transport. However, there are **0 tests** for `git.rs` or `check_drift`.
4. A concrete architecture is recommended to equip `GitManager` with state tracking (`Arc<Mutex<Option<MerkleTree>>>`, `Arc<Mutex<Vec<PathBuf>>>`, `Arc<Mutex<HashSet<PathBuf>>>`), enabling `check_drift` to detect unauthored external modifications and return `Err("ConflictDetected")` while keeping the `&self` receiver signature required by `apply_patch`.

---

## 2. Code Locations & Struct Analysis

### 2.1 `aura-core/src/knowledge/git.rs`

* **File Location:** `/home/jeff/aura/aura-core/src/knowledge/git.rs`
* **Module Hierarchy:** Exported in `aura-core/src/knowledge/mod.rs` via `pub mod git; pub use git::*;` and exposed through `aura_core::knowledge::git::GitManager` or `aura_core::knowledge::GitManager`.
* **Current Code Listing (lines 1–43):**

```rust
/// Manages Git branch isolation, drift detection, and sandboxed linting
pub struct GitManager {
    pub current_branch: String,
}

impl GitManager {
    pub fn new() -> Self {
        Self {
            current_branch: String::from("main"),
        }
    }
    /// Recalculates BLAKE3 tree before patch to detect concurrent human modifications
    pub fn check_drift(&self) -> Result<(), &'static str> {
        // Return ConflictDetected error if drifted
        Ok(())
    }

    /// Spins up a Firecracker Bash microVM to execute linting hooks
    pub fn sandbox_lint(&self) -> Result<(), String> {
        // Run ruff, prettier, cargo fmt inside VM
        Ok(())
    }

    pub fn apply_patch(&self, _patch: &str) -> Result<(), String> {
        self.check_drift().map_err(|e| e.to_string())?;
        self.sandbox_lint()?;
        // commit patch
        Ok(())
    }

    /// Implement Ephemeral Tunnel integration for opening Pull Requests
    pub fn open_pr(&self, title: &str, _body: &str) -> Result<String, String> {
        // The agent will invoke this to create a PR automatically when complete or when a drift is detected.
        Ok(format!("Opened PR: {}", title))
    }
}

impl Default for GitManager {
    fn default() -> Self {
        Self::new()
    }
}
```

#### Observations on `git.rs`:
* `GitManager` holds only `current_branch: String`. It has no knowledge of repository paths, tracked files, baseline trees, or agent authored changes.
* `check_drift(&self) -> Result<(), &'static str>` takes an immutable reference `&self` and returns a static string slice on error (`&'static str`).
* The comment on line 14 specifically specifies: `// Return ConflictDetected error if drifted`.
* Line 25 in `apply_patch` invokes `self.check_drift().map_err(|e| e.to_string())?`. This proves that any error returned by `check_drift` will immediately halt `apply_patch` before `sandbox_lint` or patch application can proceed.
* No unit tests or integration tests exist in `git.rs`.

---

## 3. Deep Dive: `indexer.rs` and `MerkleTree`

* **File Location:** `/home/jeff/aura/aura-core/src/knowledge/indexer.rs`
* **Dependencies:** `rayon` (parallel iterators), `blake3` (SIMD-accelerated cryptographic hash), `std::collections::HashMap`, `std::path::{Path, PathBuf}`.

### 3.1 Type Definitions and Methods

```rust
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
```

### 3.2 Key Operational Mechanics:
1. **Parallel Hashing:** Files are hashed concurrently using Rayon `par_iter()`. Unreadable or missing files are safely filtered out via `.ok()`.
2. **Deterministic Root Digestion:** Hashes are updated sequentially into `blake3::Hasher::new()`. Rayon's `par_iter().collect()` guarantees sequence order matching the input slice `files`.
3. **Change Detection via `update(&mut self, files: &[PathBuf]) -> Vec<PathBuf>`:**
   - It hashes every file in the provided slice.
   - If `self.file_hashes.get(&p) != Some(&h)`, the path is appended to `changed_files`. This detects:
     - Files whose content has been edited.
     - New files that were not present in the initial `build` pass.
   - It updates `self.root_hash` and `self.file_hashes` in-place.
   - It returns the `Vec<PathBuf>` containing all modified paths.
4. **Performance SLA:**
   - `test_single_file_modification_reindex_under_50ms` (lines 98–151) verifies that re-indexing across 50 files takes well under the 50ms SLA. Local execution measured this in microseconds.

---

## 4. Author Tracking and External Modification Detection

### 4.1 Problem Definition
In the Aura multi-agent operating environment:
* An agent operates concurrently with potential human developers and other background workflows.
* As the agent prepares changes (e.g., editing `file_a.rs`), an external human developer might concurrently commit edits to `file_b.rs` or `file_a.rs`.
* According to `aura-repo-domain/SKILL.md`:
  > **Conflict**: Recalculate BLAKE3 tree before patch apply to detect concurrent commits.
* Requirement **R1** from `ORIGINAL_REQUEST.md`:
  > Complete `check_drift` to use `MerkleTree::update()` (from `indexer.rs`) to compare BLAKE3 hashes and detect changes not authored by the agent.

### 4.2 Drift Detection Logic
1. **Baseline Snapshot:**
   When `GitManager` is initialized or starts tracking files, a baseline `MerkleTree` is constructed:
   ```rust
   let tree = MerkleTree::build(&tracked_files);
   ```
2. **Tracking Agent-Authored Modifications:**
   The agent records files it intentionally touches (e.g. via `record_agent_change(&path)` or when applying edits):
   ```rust
   agent_authored_files.insert(path);
   ```
3. **Evaluating Drift in `check_drift`:**
   When `check_drift()` is invoked:
   ```rust
   let changed_files = tree.update(&tracked_files);
   for file in changed_files {
       if !agent_authored_files.contains(&file) {
           return Err("ConflictDetected");
       }
   }
   Ok(())
   ```
4. **Outcomes:**
   * **Case 1: No files modified.** `changed_files` is empty $\rightarrow$ `Ok(())`.
   * **Case 2: Only agent-authored files modified.** Every entry in `changed_files` exists in `agent_authored_files` $\rightarrow$ `Ok(())`.
   * **Case 3: External modification detected.** An unauthored file was modified or an external human concurrently changed an unrecorded file $\rightarrow$ `Err("ConflictDetected")`.
   * **Case 4: Mixed modification.** Agent modified file A, but external actor modified file B $\rightarrow$ `Err("ConflictDetected")`.

---

## 5. Test Suite Analysis & Gap Identification

### 5.1 Current Test Suite Status
Running `cargo test --package aura-core` executes 74 tests across the following modules:

| Subsystem Module | Test Count | Status | Description |
|---|---|---|---|
| `config` | 11 | PASS | Configuration loading, TOML/YAML parsing, bounds validation |
| `hypervisor::firecracker` | 7 | PASS | VM lifecycle, path schemas, vsock listeners, PNG zero-trust |
| `hypervisor::proxy` | 7 | PASS | Egress SNI enforcement, domain allowlisting |
| `hypervisor::wasm` | 3 | PASS | WASI capability enforcement, fuel metering trap |
| `hypervisor::mod` | 2 | PASS | WASM / Firecracker hypervisor initialization |
| `identity` | 10 | PASS | SVID attestation, scope non-escalation, Cedar policy engine |
| `bus` | 4 | PASS | Zenoh pub/sub, task dispatch, token monitoring, rate limiter |
| `knowledge::cache` | 4 | PASS | Vector embeddings, exact cache lookup, sqlite-vec |
| `knowledge::cst` | 2 | PASS | Tree-sitter AST parsing for Rust and Python |
| `knowledge::graph` | 1 | PASS | SQLite knowledge graph and tables |
| `knowledge::indexer` | 2 | PASS | Merkle tree generation, 50ms incremental reindex SLA |
| `knowledge::resolver` | 3 | PASS | Three-pass resolver, blast radius recursive CTE |
| `runtime::registry` | 3 | PASS | Sandbox registry resolve and load |
| `supervisor` | 3 | PASS | Process spawn, lifecycle drain, task registration |
| `transport` | 12 | PASS | HTTP/3 QUIC transport, webhooks, rate limiter |
| `lib.rs` | 1 | PASS | Microkernel init integration test |
| **`knowledge::git`** | **0** | **MISSING** | **No tests exist currently** |

### 5.2 Tests Required for Acceptance Criteria

To satisfy the Acceptance Criteria:
> `cargo test` in `aura-core` passes, including new tests that verify `check_drift` correctly detects external modifications using the Merkle tree.
> `cargo test` passes for new tests verifying that `apply_patch` halts if `check_drift` fails or if `sandbox_lint` fails.

The following test suite must be added to `aura-core/src/knowledge/git.rs`:

1. `test_check_drift_no_drift`:
   - Initialize `GitManager` with two clean files.
   - Call `check_drift()`.
   - Assert returns `Ok(())`.
2. `test_check_drift_detects_external_modification`:
   - Initialize `GitManager` with two files.
   - Modify one file externally using `std::fs::write(...)`.
   - Call `check_drift()`.
   - Assert returns `Err("ConflictDetected")`.
3. `test_check_drift_permits_agent_authored_modifications`:
   - Initialize `GitManager` with two files.
   - Register one file as authored by agent using `git_manager.record_agent_change(&file1)`.
   - Modify `file1`.
   - Call `check_drift()`.
   - Assert returns `Ok(())`.
4. `test_check_drift_detects_unauthored_alongside_authored_modifications`:
   - Register `file1` as authored.
   - Modify `file1` and modify `file2` (unregistered).
   - Call `check_drift()`.
   - Assert returns `Err("ConflictDetected")`.
5. `test_apply_patch_halts_when_drift_detected`:
   - Create external modification on tracked file.
   - Call `git_manager.apply_patch("diff...")`.
   - Assert returns `Err(err)` where `err.contains("ConflictDetected")`.
   - Verify that subsequent patch commits did not occur.
6. `test_apply_patch_halts_when_sandbox_lint_fails`:
   - (Coordinates with Explorer 2) When `sandbox_lint` returns an error, `apply_patch` immediately halts and propagates the error.

---

## 6. Recommended Implementation Strategy

### 6.1 Design Principles
1. **Zero Breaking Changes:** Keep existing public signatures:
   - `pub fn new() -> Self`
   - `pub fn check_drift(&self) -> Result<(), &'static str>`
   - `pub fn apply_patch(&self, patch: &str) -> Result<(), String>`
   - `pub fn sandbox_lint(&self) -> Result<(), String>`
   - `pub fn open_pr(&self, title: &str, body: &str) -> Result<String, String>`
2. **Interior Mutability via Thread-Safe Mutexes:**
   Because `check_drift(&self)` takes `&self`, but `MerkleTree::update(&mut self, ...)` requires `&mut self`, we wrap the internal tree and tracking structures in `Arc<Mutex<...>>`. This also ensures safety across multithreaded Tokio and Rayon workloads.
3. **Canonical Error Constants:**
   Define `pub const CONFLICT_DETECTED: &'static str = "ConflictDetected";` so tests and callers can compare cleanly.

### 6.2 Proposed Code Architecture for `aura-core/src/knowledge/git.rs`

```rust
use crate::knowledge::indexer::MerkleTree;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub const CONFLICT_DETECTED: &str = "ConflictDetected";

/// Manages Git branch isolation, drift detection, and sandboxed linting
#[derive(Clone)]
pub struct GitManager {
    pub current_branch: String,
    pub repo_root: Option<PathBuf>,
    pub tracked_files: Arc<Mutex<Vec<PathBuf>>>,
    pub merkle_tree: Arc<Mutex<Option<MerkleTree>>>,
    pub agent_authored_files: Arc<Mutex<HashSet<PathBuf>>>,
}

impl GitManager {
    pub fn new() -> Self {
        Self {
            current_branch: String::from("main"),
            repo_root: None,
            tracked_files: Arc::new(Mutex::new(Vec::new())),
            merkle_tree: Arc::new(Mutex::new(None)),
            agent_authored_files: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Initializes baseline Merkle tree for tracked files.
    pub fn init_baseline(&self, files: &[PathBuf]) {
        let mut tracked = self.tracked_files.lock().unwrap();
        *tracked = files.to_vec();
        let tree = MerkleTree::build(files);
        let mut tree_guard = self.merkle_tree.lock().unwrap();
        *tree_guard = Some(tree);
    }

    /// Builder helper to initialize with tracked files.
    pub fn with_files(self, files: &[PathBuf]) -> Self {
        self.init_baseline(files);
        self
    }

    /// Records a file change as authored by the agent.
    pub fn record_agent_change(&self, path: impl AsRef<Path>) {
        let mut authored = self.agent_authored_files.lock().unwrap();
        authored.insert(path.as_ref().to_path_buf());
    }

    /// Recalculates BLAKE3 tree before patch to detect concurrent human/external modifications.
    pub fn check_drift(&self) -> Result<(), &'static str> {
        let mut tree_guard = self
            .merkle_tree
            .lock()
            .map_err(|_| "Failed to acquire lock on Merkle tree")?;

        if let Some(ref mut tree) = *tree_guard {
            let tracked = self
                .tracked_files
                .lock()
                .map_err(|_| "Failed to acquire lock on tracked files")?;

            let changed_files = tree.update(&tracked);
            let authored = self
                .agent_authored_files
                .lock()
                .map_err(|_| "Failed to acquire lock on authored files")?;

            for changed in changed_files {
                if !authored.contains(&changed) {
                    return Err(CONFLICT_DETECTED);
                }
            }
        }

        Ok(())
    }

    /// Spins up a Firecracker Bash microVM to execute linting hooks
    pub fn sandbox_lint(&self) -> Result<(), String> {
        // Implementation detailed in Explorer 2 survey report
        Ok(())
    }

    pub fn apply_patch(&self, patch: &str) -> Result<(), String> {
        self.check_drift().map_err(|e| e.to_string())?;
        self.sandbox_lint()?;
        // commit patch
        Ok(())
    }

    /// Implement Ephemeral Tunnel integration for opening Pull Requests
    pub fn open_pr(&self, title: &str, _body: &str) -> Result<String, String> {
        // Implementation detailed in Explorer 2 survey report
        Ok(format!("Opened PR: {}", title))
    }
}

impl Default for GitManager {
    fn default() -> Self {
        Self::new()
    }
}
```

---

## 7. Dependencies & Coordination Notes

1. **Internal Domain Boundaries:**
   - `aura-core::knowledge::git` directly depends on `aura-core::knowledge::indexer::MerkleTree`.
   - Both are inside the `knowledge` module and share `rayon` and `blake3` dependencies without needing extra crates.
2. **Coordination with Explorer 2 (Sandbox Lint & Ephemeral Tunnel):**
   - Explorer 2 is surveying `sandbox_lint` (Firecracker microVM invocation) and `open_pr` (Ephemeral Tunnel credential minting).
   - The proposed `GitManager` struct easily accommodates both since `apply_patch` sequences `check_drift` first, followed by `sandbox_lint`.
3. **Downstream Tasks for Implementer:**
   - Update `aura-core/src/knowledge/git.rs` with the state-tracked `GitManager`.
   - Add unit tests in `git.rs` covering all 6 test scenarios outlined in Section 5.2.
   - Run `cargo test --package aura-core` to verify all tests pass.
