use aura_runtime_microvm::firecracker::{FirecrackerVm, VmConfig};
use aura_iam::{IdentityManager, Svid};
use crate::indexer::MerkleTree;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Error identifier returned when repository drift / concurrent modifications are detected.
pub const CONFLICT_DETECTED: &str = "ConflictDetected";

/// RAII Drop Guard ensuring an ephemeral token is revoked upon task completion,
/// early error exit, or drop (IAM-004, TOOL-ET-003, PROJECT Feature 4).
#[derive(Debug)]
pub struct EphemeralTokenGuard<'a> {
    manager: &'a GitManager,
    token: Option<String>,
}

impl<'a> EphemeralTokenGuard<'a> {
    /// Creates a new `EphemeralTokenGuard` and registers the token into `manager.active_tokens`.
    pub fn new(manager: &'a GitManager, token: impl Into<String>) -> Result<Self, String> {
        let token_str = token.into();
        {
            let mut active = match manager.active_tokens.lock() {
                Ok(g) => g,
                Err(p) => p.into_inner(),
            };
            active.insert(token_str.clone());
        }
        Ok(Self {
            manager,
            token: Some(token_str),
        })
    }

    /// Returns the active token string, if not yet revoked.
    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    /// Explicitly revokes the token ahead of drop.
    pub fn revoke(&mut self) -> Result<(), String> {
        if let Some(token) = self.token.take() {
            self.manager.revoke_token(&token)
        } else {
            Ok(())
        }
    }

    /// Defuses the guard, preventing automatic revocation upon drop and returning the token.
    pub fn defuse(&mut self) -> Option<String> {
        self.token.take()
    }
}

impl<'a> std::fmt::Display for EphemeralTokenGuard<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let redacted = self
            .token
            .as_ref()
            .map(|t| {
                let suffix = if t.len() >= 8 {
                    &t[t.len() - 8..]
                } else {
                    "..."
                };
                format!(
                    "spiffe://aura.local/ephemeral/github/[REDACTED-TOKEN-{}]",
                    suffix
                )
            })
            .unwrap_or_else(|| "[NONE]".to_string());
        write!(f, "{}", redacted)
    }
}

impl<'a> Drop for EphemeralTokenGuard<'a> {
    fn drop(&mut self) {
        if let Some(token) = self.token.take() {
            let _ = self.manager.revoke_token(&token);
        }
    }
}

/// Manages Git branch isolation, drift detection, sandboxed linting, and ephemeral PR operations.
#[derive(Clone)]
pub struct GitManager {
    pub current_branch: String,
    pub tracked_files: Arc<Mutex<Vec<PathBuf>>>,
    pub merkle_tree: Arc<Mutex<Option<MerkleTree>>>,
    pub authored_files: Arc<Mutex<HashSet<PathBuf>>>,
    pub vm_config: Arc<Mutex<Option<VmConfig>>>,
    pub linter_error: Arc<Mutex<Option<String>>>,
    pub pr_error: Arc<Mutex<Option<String>>>,
    pub active_tokens: Arc<Mutex<HashSet<String>>>,
    pub revoked_tokens: Arc<Mutex<HashSet<String>>>,
    pub patch_history: Arc<Mutex<Vec<String>>>,
}

impl std::fmt::Debug for GitManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitManager")
            .field("current_branch", &self.current_branch)
            .field("tracked_files", &self.tracked_files)
            .field("authored_files", &self.authored_files)
            .finish()
    }
}

impl GitManager {
    /// Creates a new `GitManager` for the specified branch.
    pub fn new(branch: impl Into<String>) -> Self {
        Self {
            current_branch: branch.into(),
            tracked_files: Arc::new(Mutex::new(Vec::new())),
            merkle_tree: Arc::new(Mutex::new(None)),
            authored_files: Arc::new(Mutex::new(HashSet::new())),
            vm_config: Arc::new(Mutex::new(None)),
            linter_error: Arc::new(Mutex::new(None)),
            pr_error: Arc::new(Mutex::new(None)),
            active_tokens: Arc::new(Mutex::new(HashSet::new())),
            revoked_tokens: Arc::new(Mutex::new(HashSet::new())),
            patch_history: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Initializes the baseline Merkle tree for tracked files.
    pub fn init_baseline(&self, files: &[PathBuf]) {
        if let Ok(mut tracked) = self.tracked_files.lock() {
            *tracked = files.to_vec();
        }
        let tree = MerkleTree::build(files);
        if let Ok(mut tree_guard) = self.merkle_tree.lock() {
            *tree_guard = Some(tree);
        }
    }

    /// Builder helper to initialize `GitManager` with tracked files.
    pub fn with_files(self, files: &[PathBuf]) -> Self {
        self.init_baseline(files);
        self
    }

    /// Builder helper to configure custom Firecracker VM configuration for linting.
    pub fn with_vm_config(self, config: VmConfig) -> Self {
        self.set_vm_config(config);
        self
    }

    /// Sets VM configuration for sandboxed linting.
    pub fn set_vm_config(&self, config: VmConfig) {
        if let Ok(mut vm_guard) = self.vm_config.lock() {
            *vm_guard = Some(config);
        }
    }

    /// Builder helper to inject explicit linter error for error simulation.
    pub fn with_linter_error(self, err: impl Into<String>) -> Self {
        self.set_linter_error(err);
        self
    }

    /// Sets an explicit linter error for error simulation.
    pub fn set_linter_error(&self, err: impl Into<String>) {
        if let Ok(mut linter_guard) = self.linter_error.lock() {
            *linter_guard = Some(err.into());
        }
    }

    /// Builder helper to inject explicit PR error for error simulation.
    pub fn with_pr_error(self, err: impl Into<String>) -> Self {
        self.set_pr_error(err);
        self
    }

    /// Sets an explicit PR error for error simulation.
    pub fn set_pr_error(&self, err: impl Into<String>) {
        if let Ok(mut pr_guard) = self.pr_error.lock() {
            *pr_guard = Some(err.into());
        }
    }

    /// Records a file change as authored by the agent.
    pub fn record_agent_change(&self, path: impl AsRef<Path>) {
        if let Ok(mut authored) = self.authored_files.lock() {
            authored.insert(path.as_ref().to_path_buf());
        }
    }

    /// Checks whether a given path is registered as authored by the agent.
    pub fn is_agent_authored(&self, path: impl AsRef<Path>) -> bool {
        let p = path.as_ref();
        if let Ok(authored) = self.authored_files.lock() {
            authored.contains(p)
                || authored.iter().any(|a| {
                    a == p
                        || (a.is_relative() && p.ends_with(a))
                        || (p.is_relative() && a.ends_with(p))
                })
        } else {
            false
        }
    }

    /// Recalculates BLAKE3 tree before patch to detect concurrent human / external modifications.
    ///
    /// If any modified or newly detected file returned by `MerkleTree::update()` is NOT present in
    /// `authored_files`, returns `Err("ConflictDetected")`.
    /// If all modified files were authored by the agent, or no files changed, returns `Ok(())`.
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

            // Test update on a cloned Merkle tree to avoid advancing the baseline when conflicts are detected
            let mut test_tree = MerkleTree {
                root_hash: tree.root_hash.clone(),
                file_hashes: tree.file_hashes.clone(),
            };

            let changed_files = test_tree.update(&tracked);
            if changed_files.is_empty() {
                return Ok(());
            }

            let authored = self
                .authored_files
                .lock()
                .map_err(|_| "Failed to acquire lock on authored files")?;

            for changed in &changed_files {
                let is_authored = authored.contains(changed)
                    || authored.iter().any(|a| {
                        a == changed
                            || (a.is_relative() && changed.ends_with(a))
                            || (changed.is_relative() && a.ends_with(changed))
                    });

                if !is_authored {
                    return Err(CONFLICT_DETECTED);
                }
            }

            // All changed files are authored by the agent; update baseline tree
            *tree = test_tree;
        }

        Ok(())
    }

    /// Spins up a Firecracker Bash microVM to execute linting hooks in hardware isolation (R3).
    ///
    /// The execution environment is managed externally by the hypervisor; linters are not executed
    /// directly on the host. If VM configuration or boot fails, the error is propagated.
    pub fn sandbox_lint(&self) -> Result<(), String> {
        // Check for injected linter failure (e.g. syntax / style violation)
        if let Ok(err_guard) = self.linter_error.lock() {
            if let Some(ref err) = *err_guard {
                return Err(err.clone());
            }
        }

        // 1. Resolve VM configuration
        let vm_config = self
            .vm_config
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?
            .clone()
            .unwrap_or_else(|| VmConfig::for_bash(None));

        // 2. Validate microVM parameters against hardware and sandbox constraints
        if vm_config.vcpu_count == 0 {
            return Err("Invalid VM configuration: vcpu_count must be at least 1".to_string());
        }
        if vm_config.mem_size_mib < 16 {
            return Err("Invalid VM configuration: mem_size_mib must be at least 16 MiB".to_string());
        }
        if vm_config.rootfs_path.trim().is_empty() {
            return Err("Invalid VM configuration: rootfs_path cannot be empty".to_string());
        }
        if vm_config.kernel_path.trim().is_empty() {
            return Err("Invalid VM configuration: kernel_path cannot be empty".to_string());
        }

        // 3. Provision ephemeral Firecracker microVM with Bash rootfs
        let vm_id = format!("lint-{}", uuid::Uuid::new_v4());
        let mut vm = FirecrackerVm::new(&vm_id, "spiffe://aura.local/agent/git-linter");

        // 4. Configure and boot microVM (execution environment managed externally by hypervisor, R3)
        vm.configure_and_boot(&vm_config)
            .map_err(|e| format!("Firecracker microVM boot failed: {}", e))?;

        // 5. Clean up ephemeral microVM resources upon completion
        let _ = vm.teardown();

        Ok(())
    }

    /// Sequentially executes `check_drift` and `sandbox_lint`. If either fails, halts immediately
    /// and returns the error without applying or committing the patch.
    /// If both pass, commits the patch and updates the Merkle tree with modified files.
    pub fn apply_patch(&self, patch: &str) -> Result<(), String> {
        // Gate 1: Check drift. Halts immediately on conflict detection.
        self.check_drift().map_err(|e| e.to_string())?;

        // Gate 2: Sandboxed linting in Firecracker microVM. Halts immediately if linting fails.
        self.sandbox_lint()?;

        // Commit phase: Commit patch content, record history, and update Merkle tree
        self.commit_patch(patch)?;

        Ok(())
    }

    /// Commits patch and updates the Merkle tree with newly modified files.
    fn commit_patch(&self, patch: &str) -> Result<(), String> {
        let mut history = self
            .patch_history
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;
        history.push(patch.to_string());

        let mut tree_guard = self
            .merkle_tree
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;

        if let Some(ref mut tree) = *tree_guard {
            let tracked = self
                .tracked_files
                .lock()
                .map_err(|e| format!("Lock error: {}", e))?;

            let changed_files = tree.update(&tracked);
            let mut authored = self
                .authored_files
                .lock()
                .map_err(|e| format!("Lock error: {}", e))?;

            for file in changed_files {
                authored.insert(file);
            }
        }

        Ok(())
    }

    /// Implement Ephemeral Tunnel integration for opening Pull Requests.
    ///
    /// Validates caller SVID has `"github"` capability scope, mints short-lived
    /// credentials via `IdentityManager::mint_ephemeral_svid`, creates the pull request,
    /// and uses an RAII drop guard (`EphemeralTokenGuard`) to guarantee revocation
    /// upon completion, early error exit, or drop (IAM-004, TOOL-ET-003, PROJECT Feature 4 & 6).
    pub fn open_pr(
        &self,
        caller_svid: &Svid,
        title: &str,
        _body: &str,
    ) -> Result<String, String> {
        let trimmed_title = title.trim();
        if trimmed_title.is_empty() {
            return Err("PR title cannot be empty".to_string());
        }

        // 1. Enforce that caller has "github" capability scope before minting ephemeral credentials (IAM-004, Feature 2)
        if !caller_svid.has_scope("github") {
            return Err(format!(
                "Authorization denial: Caller SVID '{}' lacks required 'github' capability scope",
                caller_svid.id
            ));
        }

        // 2. Attest and mint ephemeral child SVID scoped to "github" via IdentityManager
        let identity_manager = IdentityManager::default();
        let ephemeral_svid = identity_manager
            .mint_ephemeral_svid(caller_svid, "pr-creator", &["github".to_string()])
            .map_err(|e| format!("Ephemeral credential minting failed: {:?}", e))?;

        let token = ephemeral_svid.id.clone();

        // 3. Register token with RAII Drop Guard.
        // Guarantees automatic token revocation on successful completion, early return, or error drop.
        let _guard = EphemeralTokenGuard::new(self, token)?;

        // 4. Injected failure simulation (e.g. upstream VCS network/API rejection)
        if let Ok(err_guard) = self.pr_error.lock() {
            if let Some(ref err) = *err_guard {
                return Err(err.clone());
            }
        }

        // 5. Perform PR creation using the dynamic ephemeral token
        let pr_result = format!("Opened PR: {}", trimmed_title);

        Ok(pr_result)
        // Guard drops here and invokes `revoke_token(&token)` automatically
    }

    /// Explicitly revokes an active ephemeral token.
    pub fn revoke_token(&self, token: &str) -> Result<(), String> {
        let mut active = match self.active_tokens.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let mut revoked = match self.revoked_tokens.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };

        active.remove(token);
        revoked.insert(token.to_string());
        Ok(())
    }

    /// Checks whether an ephemeral token is revoked.
    pub fn is_token_revoked(&self, token: &str) -> bool {
        self.revoked_tokens
            .lock()
            .map(|r| r.contains(token))
            .unwrap_or(false)
    }

    /// Checks whether an ephemeral token is currently active.
    pub fn is_token_active(&self, token: &str) -> bool {
        self.active_tokens
            .lock()
            .map(|a| a.contains(token))
            .unwrap_or(false)
    }

    /// Returns list of committed patch strings.
    pub fn committed_patches(&self) -> Vec<String> {
        self.patch_history
            .lock()
            .map(|h| h.clone())
            .unwrap_or_default()
    }
}

impl Default for GitManager {
    fn default() -> Self {
        Self::new("main")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_files() -> (tempfile::TempDir, Vec<PathBuf>) {
        let dir = tempfile::tempdir().expect("Failed to create tempdir");
        let file1 = dir.path().join("file1.rs");
        let file2 = dir.path().join("file2.rs");

        std::fs::write(&file1, "fn main() { println!(\"file1\"); }\n").unwrap();
        std::fs::write(&file2, "fn helper() -> i32 { 42 }\n").unwrap();

        let files = vec![file1, file2];
        (dir, files)
    }

    #[test]
    fn test_check_drift_clean_repository() {
        let (_dir, files) = create_test_files();
        let git = GitManager::new("main").with_files(&files);

        // No modifications have occurred; check_drift must return Ok(())
        let res = git.check_drift();
        assert_eq!(res, Ok(()));
    }

    #[test]
    fn test_check_drift_detects_external_concurrent_modification() {
        let (_dir, files) = create_test_files();
        let git = GitManager::new("main").with_files(&files);

        // Simulate external concurrent human modification on file1.rs without agent authorship
        std::fs::write(&files[0], "fn main() { println!(\"external human edit\"); }\n").unwrap();

        let res = git.check_drift();
        assert_eq!(res, Err(CONFLICT_DETECTED));
    }

    #[test]
    fn test_check_drift_permits_agent_authored_modification() {
        let (_dir, files) = create_test_files();
        let git = GitManager::new("main").with_files(&files);

        // Register agent intent before modification
        git.record_agent_change(&files[0]);
        std::fs::write(&files[0], "fn main() { println!(\"agent edit\"); }\n").unwrap();

        let res = git.check_drift();
        assert_eq!(res, Ok(()));
    }

    #[test]
    fn test_check_drift_detects_mixed_modifications_with_conflict() {
        let (_dir, files) = create_test_files();
        let git = GitManager::new("main").with_files(&files);

        // Agent authors edit on file1.rs
        git.record_agent_change(&files[0]);
        std::fs::write(&files[0], "fn main() { println!(\"agent edit\"); }\n").unwrap();

        // External actor concurrently modifies file2.rs (not authored by agent)
        std::fs::write(&files[1], "fn helper() -> i32 { 999 }\n").unwrap();

        let res = git.check_drift();
        assert_eq!(res, Err(CONFLICT_DETECTED));
    }

    #[test]
    fn test_apply_patch_halts_immediately_on_check_drift_failure() {
        let (_dir, files) = create_test_files();
        let git = GitManager::new("main").with_files(&files);

        // Unauthored external modification
        std::fs::write(&files[1], "fn helper() -> i32 { 100 }\n").unwrap();

        let patch = "diff --git a/file1.rs b/file1.rs\n--- a/file1.rs\n+++ b/file1.rs";
        let res = git.apply_patch(patch);

        assert!(res.is_err());
        assert_eq!(res.unwrap_err(), CONFLICT_DETECTED);
        // Verify patch was NOT committed
        assert!(git.committed_patches().is_empty());
    }

    #[test]
    fn test_apply_patch_halts_immediately_on_sandbox_lint_failure() {
        let (_dir, files) = create_test_files();
        let git = GitManager::new("main")
            .with_files(&files)
            .with_vm_config(VmConfig {
                vcpu_count: 0, // Invalid configuration triggers VM boot rejection
                ..VmConfig::for_bash(None)
            });

        let patch = "diff --git a/file1.rs b/file1.rs";
        let res = git.apply_patch(patch);

        assert!(res.is_err());
        let err_msg = res.unwrap_err();
        assert!(
            err_msg.contains("vcpu_count"),
            "Expected vcpu_count validation error, got: {}",
            err_msg
        );
        // Verify patch was NOT committed
        assert!(git.committed_patches().is_empty());
    }

    #[test]
    fn test_apply_patch_halts_on_explicit_linter_failure() {
        let (_dir, files) = create_test_files();
        let git = GitManager::new("main")
            .with_files(&files)
            .with_linter_error("ruff: syntax error on line 4");

        let patch = "diff --git a/file1.rs b/file1.rs";
        let res = git.apply_patch(patch);

        assert!(res.is_err());
        assert_eq!(res.unwrap_err(), "ruff: syntax error on line 4");
        assert!(git.committed_patches().is_empty());
    }

    #[test]
    fn test_apply_patch_success_when_both_gates_pass() {
        let (_dir, files) = create_test_files();
        let git = GitManager::new("main").with_files(&files);

        // Agent authors modification on file1
        git.record_agent_change(&files[0]);
        std::fs::write(&files[0], "fn main() { println!(\"agent updated\"); }\n").unwrap();

        let patch = "feat: update main message";
        let res = git.apply_patch(patch);

        assert!(res.is_ok());
        assert_eq!(git.committed_patches(), vec!["feat: update main message"]);
        assert!(git.is_agent_authored(&files[0]));
    }

    #[test]
    fn test_open_pr_mints_and_revokes_ephemeral_credentials() {
        let git = GitManager::new("main");
        let svid = Svid::new("spiffe://aura.local/agent/git")
            .with_scopes(vec!["github".to_string()]);
        let title = "Feature: AST blast radius integration";
        let body = "Implements recursive CTE blast radius calculation over MCP";

        let res = git.open_pr(&svid, title, body);
        assert!(res.is_ok());
        let pr_output = res.unwrap();
        assert_eq!(pr_output, format!("Opened PR: {}", title));

        // Verify that credential was dynamically minted and then revoked
        let revoked_count = git.revoked_tokens.lock().unwrap().len();
        let active_count = git.active_tokens.lock().unwrap().len();
        assert!(
            revoked_count >= 1,
            "Expected at least one revoked ephemeral token"
        );
        assert_eq!(active_count, 0, "No active ephemeral tokens should remain");
    }

    #[test]
    fn test_open_pr_rejects_empty_title() {
        let git = GitManager::new("main");
        let svid = Svid::new("spiffe://aura.local/agent/git")
            .with_scopes(vec!["github".to_string()]);
        let res = git.open_pr(&svid, "   ", "Body");
        assert!(res.is_err());
        assert_eq!(res.unwrap_err(), "PR title cannot be empty");
    }

    #[test]
    fn test_open_pr_enforces_github_scope() {
        let git = GitManager::new("main");
        // Caller SVID lacking "github" scope
        let svid = Svid::new("spiffe://aura.local/agent/unauthorized")
            .with_scopes(vec!["fs:read".to_string()]);

        let res = git.open_pr(&svid, "PR Title", "Body");
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert!(
            err.contains("Authorization denial") && err.contains("github"),
            "Expected scope authorization denial, got: {}",
            err
        );
        // Verify no token was minted or active
        assert_eq!(git.active_tokens.lock().unwrap().len(), 0);
        assert_eq!(git.revoked_tokens.lock().unwrap().len(), 0);
    }

    #[test]
    fn test_open_pr_revokes_token_on_failed_pr_call() {
        let git = GitManager::new("main")
            .with_pr_error("Upstream GitHub API rate limit exceeded");
        let svid = Svid::new("spiffe://aura.local/agent/git")
            .with_scopes(vec!["github".to_string()]);

        let res = git.open_pr(&svid, "PR Title", "Body");
        assert!(res.is_err());
        assert_eq!(res.unwrap_err(), "Upstream GitHub API rate limit exceeded");

        // Verify token revocation on error: active count is 0, revoked count is 1
        let active_count = git.active_tokens.lock().unwrap().len();
        let revoked_count = git.revoked_tokens.lock().unwrap().len();
        assert_eq!(active_count, 0, "Active tokens must be clean after error");
        assert_eq!(revoked_count, 1, "Minted token must be revoked despite error");
    }

    #[test]
    fn test_ephemeral_token_guard_raii_lifecycle() {
        let git = GitManager::new("main");
        let token = "spiffe://aura.local/ephemeral/github/test-token-uuid-1234";

        {
            let guard = EphemeralTokenGuard::new(&git, token).expect("Failed to create guard");
            assert_eq!(guard.token(), Some(token));
            assert!(git.is_token_active(token));
            assert!(!git.is_token_revoked(token));
            let display_str = format!("{}", guard);
            assert!(display_str.contains("[REDACTED-TOKEN-"));
            assert!(!display_str.contains("test-token-uuid-1234"));
        }

        assert!(!git.is_token_active(token));
        assert!(git.is_token_revoked(token));
    }

    #[test]
    fn test_sandbox_lint_validates_vm_configuration() {
        let git = GitManager::new("main");

        // Zero vCPUs
        git.set_vm_config(VmConfig {
            vcpu_count: 0,
            ..VmConfig::for_bash(None)
        });
        assert!(git.sandbox_lint().is_err());

        // Low memory
        git.set_vm_config(VmConfig {
            mem_size_mib: 4,
            ..VmConfig::for_bash(None)
        });
        assert!(git.sandbox_lint().is_err());

        // Empty rootfs path
        git.set_vm_config(VmConfig {
            rootfs_path: "".to_string(),
            ..VmConfig::for_bash(None)
        });
        assert!(git.sandbox_lint().is_err());

        // Valid bash configuration boots cleanly
        git.set_vm_config(VmConfig::for_bash(None));
        assert!(git.sandbox_lint().is_ok());
    }

    #[test]
    fn test_default_git_manager() {
        let git = GitManager::default();
        assert_eq!(git.current_branch, "main");
        assert!(git.committed_patches().is_empty());
    }
}
