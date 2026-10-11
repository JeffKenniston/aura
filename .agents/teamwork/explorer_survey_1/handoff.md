# Handoff Report: Survey Explorer 1 (Git Drift & MerkleTree)

## 1. Observation

1. **`git.rs` Location & Content**:
   - File Path: `/home/jeff/aura/aura-core/src/knowledge/git.rs` (lines 1–43)
   - Current struct definition:
     ```rust
     pub struct GitManager {
         pub current_branch: String,
     }
     ```
   - Current stub for `check_drift`:
     ```rust
     /// Recalculates BLAKE3 tree before patch to detect concurrent human modifications
     pub fn check_drift(&self) -> Result<(), &'static str> {
         // Return ConflictDetected error if drifted
         Ok(())
     }
     ```
   - Current `apply_patch`:
     ```rust
     pub fn apply_patch(&self, _patch: &str) -> Result<(), String> {
         self.check_drift().map_err(|e| e.to_string())?;
         self.sandbox_lint()?;
         // commit patch
         Ok(())
     }
     ```
   - Exported at `/home/jeff/aura/aura-core/src/knowledge/mod.rs:3`: `pub mod git; pub use git::*;`.

2. **`indexer.rs` MerkleTree Implementation**:
   - File Path: `/home/jeff/aura/aura-core/src/knowledge/indexer.rs` (lines 12–62)
   - `MerkleTree` struct:
     ```rust
     pub struct MerkleTree {
         pub root_hash: String,
         pub file_hashes: HashMap<PathBuf, String>,
     }
     ```
   - `MerkleTree::build(files: &[PathBuf]) -> Self` (lines 19–37): Computes BLAKE3 hash for files via `rayon::prelude::*` parallel iterators and digests into root hash.
   - `MerkleTree::update(&mut self, files: &[PathBuf]) -> Vec<PathBuf>` (lines 39–61): Re-hashes files in parallel, detects modified/new files where `self.file_hashes.get(&p) != Some(&h)`, updates internal hashes, and returns `Vec<PathBuf>` of changed files.
   - Existing performance test: `test_single_file_modification_reindex_under_50ms` (lines 98–151) enforces < 50ms re-index SLA.

3. **Existing Test Suite in `aura-core`**:
   - Command run: `cargo test --package aura-core`
   - Result: 74 passed; 0 failed; 0 ignored; finished in 1.25s.
   - Test files checked: `src/bus/`, `src/config/`, `src/hypervisor/`, `src/identity/`, `src/knowledge/`, `src/runtime/`, `src/supervisor/`, `src/transport/`, `src/lib.rs`.
   - `aura-core/src/knowledge/git.rs` has **0 unit tests**.

4. **Relevant Domain Directives**:
   - `/home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-repo-domain/SKILL.md`:
     "Conflict: Recalculate BLAKE3 tree before patch apply to detect concurrent commits."
   - `/home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md`:
     "Complete `check_drift` to use `MerkleTree::update()` (from `indexer.rs`) to compare BLAKE3 hashes and detect changes not authored by the agent."
     "Acceptance criteria: `cargo test` in `aura-core` passes, including new tests that verify `check_drift` correctly detects external modifications using the Merkle tree."
     "Acceptance criteria: `cargo test` passes for new tests verifying that `apply_patch` halts if `check_drift` fails or if `sandbox_lint` fails."

---

## 2. Logic Chain

1. From Observation 1, `GitManager` currently possesses only `pub current_branch: String`, without any fields to store tracked file paths, the baseline `MerkleTree`, or the set of files authored by the agent.
2. From Observation 1, `check_drift(&self)` returns `Result<(), &'static str>` and takes an immutable reference `&self`. However, from Observation 2, `MerkleTree::update(&mut self, files: &[PathBuf])` requires a mutable reference (`&mut self`) on the tree.
3. Therefore, to reconcile the immutable `&self` receiver of `check_drift(&self)` and `apply_patch(&self)` with the mutable `update(&mut self)` requirement, `GitManager` must employ thread-safe interior mutability (such as `Arc<Mutex<Option<MerkleTree>>>` and `Arc<Mutex<HashSet<PathBuf>>>`).
4. From Observation 4, drift detection requires comparing updated file hashes against changes authored by the agent. When `tree.update(&tracked_files)` returns `changed_files: Vec<PathBuf>`, if any changed file is not present in `agent_authored_files`, an external modification (drift/conflict) has occurred.
5. In such an event, `check_drift()` must return `Err("ConflictDetected")`. This satisfies the line 14 comment (`// Return ConflictDetected error if drifted`).
6. From Observation 1 (`apply_patch`), line 25 executes `self.check_drift().map_err(|e| e.to_string())?`. Returning `Err("ConflictDetected")` ensures that `apply_patch` immediately halts and propagates the conflict error as requested by the Acceptance Criteria.
7. From Observation 3, there are currently 0 tests for `git.rs`. Thus, unit tests covering:
   - clean baseline (no drift $\rightarrow$ `Ok(())`),
   - unrecorded external edits (drift detected $\rightarrow$ `Err("ConflictDetected")`),
   - recorded agent-authored edits (no drift $\rightarrow$ `Ok(())`),
   - mixed edits (unrecorded edit detected alongside authored edit $\rightarrow$ `Err("ConflictDetected")`), and
   - `apply_patch` gating (halting on drift)
   must be introduced in `git.rs` under `#[cfg(test)] mod tests`.

---

## 3. Caveats

1. **`sandbox_lint` & `open_pr` Scope Separation**:
   This survey focused strictly on `git.rs` structure, `indexer.rs`, BLAKE3 Merkle trees, and `check_drift`. Detailed Firecracker microVM invocation for `sandbox_lint` and Ephemeral Tunnel credential minting for `open_pr` are being surveyed in parallel by Survey Explorer 2.
2. **File System Deletions**:
   `MerkleTree::update(&mut self, files: &[PathBuf])` re-hashes provided `files`. If an external user deletes a file from disk, `update` only returns changed files from the slice provided. If whole-directory file deletion tracking is required in the future, `files` should be enumerated dynamically from disk or compared against `self.file_hashes.keys()`.

---

## 4. Conclusion

1. `check_drift` can be fully implemented using the existing `MerkleTree::update()` in `aura-core/src/knowledge/indexer.rs` without adding any new external dependencies.
2. `GitManager` should be enhanced with:
   - `pub tracked_files: Arc<Mutex<Vec<PathBuf>>>`
   - `pub merkle_tree: Arc<Mutex<Option<MerkleTree>>>`
   - `pub agent_authored_files: Arc<Mutex<HashSet<PathBuf>>>`
   - Helper methods: `init_baseline(&self, files: &[PathBuf])`, `with_files(self, files: &[PathBuf]) -> Self`, and `record_agent_change(&self, path: impl AsRef<Path>)`.
3. `check_drift(&self)` should return `Err("ConflictDetected")` whenever `tree.update()` finds a changed file not present in `agent_authored_files`, and `Ok(())` otherwise.
4. Comprehensive unit tests should be added to `aura-core/src/knowledge/git.rs` testing drift detection, author tracking, and `apply_patch` gating.
5. Full implementation proposals and signatures are detailed in `/home/jeff/aura/.agents/teamwork/explorer_survey_1/survey_report.md`.

---

## 5. Verification Method

To independently verify all observations and conclusions:

1. **Verify `git.rs` stubs**:
   ```bash
   cat /home/jeff/aura/aura-core/src/knowledge/git.rs
   ```
2. **Verify `indexer.rs` Merkle tree implementation and tests**:
   ```bash
   cargo test --package aura-core knowledge::indexer::tests
   ```
3. **Verify current `aura-core` baseline tests**:
   ```bash
   cargo test --package aura-core
   ```
   (Should report 74 passed tests with 0 failures).
4. **Invalidation Condition**:
   If modifying `GitManager`'s fields breaks any existing public call site or fails `cargo test --package aura-core`, the conclusion is invalidated. (Verified: `GitManager` has 0 external callers in the workspace).
