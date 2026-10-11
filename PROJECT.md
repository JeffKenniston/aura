# Project: M4 Repo Intelligence (Graph, Blast-Radius, Git Stubs)

## Architecture
- **aura-core**: Microkernel core crate providing:
  - `knowledge::git::GitManager`: Git workspace orchestration, Merkle-tree drift detection, microVM sandboxed linting, PR creation via ephemeral credentials, and atomic patch application.
  - `knowledge::indexer::MerkleTree`: BLAKE3 cryptographic hash tree indexing tracked files with sub-50ms parallel delta computation via `MerkleTree::update()`.
  - `hypervisor::firecracker::FirecrackerVm`: Firecracker microVM launcher and lifecycle manager providing hardware-isolated lint execution without host contamination (R3).
  - `identity::IdentityManager` / `tools::ephemeral-tunnel`: SPIFFE/SPIRE short-lived SVID and credential minting for GitHub API interactions.
- **tools/ast-blast-radius**: WASM component in `tools/` sub-workspace providing:
  - `cte::CallGraph`: Recursive CTE graph traversal with cycle detection.
  - `risk::RiskScorer`: Risk calculation with blast radius threshold (> 10 symbols) and safety score limits (< 70).
  - `mcp`: JSON-RPC 2.0 MCP protocol handling (`tools/call`, `tools/list`) returning structured AST blast radius reports.

## Feature Inventory
| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| 1 | `check_drift` | Use `MerkleTree::update()` from `indexer.rs` to compare BLAKE3 hashes and detect changes not authored by agent, returning `Err("ConflictDetected")` | M4.1 | ORIGINAL_REQUEST §R1 |
| 2 | `sandbox_lint` | Launch Firecracker microVM (`hypervisor/firecracker.rs`) with `VmConfig::for_bash` to execute repository linters (ruff, prettier, cargo fmt) in microVM | M4.1 | ORIGINAL_REQUEST §R1, §R3 |
| 3 | `open_pr` | Use Ephemeral Tunnel / IdentityManager to securely mint and revoke short-lived credentials for creating Pull Requests | M4.1 | ORIGINAL_REQUEST §R1 |
| 4 | `apply_patch` | Sequentially execute `check_drift` and `sandbox_lint`, halting if either fails, before committing changes | M4.1 | ORIGINAL_REQUEST §R1 |
| 5 | `ast-blast-radius` MCP Wiring | Implement JSON-RPC 2.0 MCP handler (`mcp.rs`) supporting `tools/call` (with `symbol_id` or `target_symbol_id`) and `tools/list` | M4.2 | ORIGINAL_REQUEST §R2 |
| 6 | Recursive CTE Blast Report | Execute recursive CTE graph traversal, compute risk score and blast radius, and format as MCP result | M4.2 | ORIGINAL_REQUEST §R2 |
| 7 | Objective Verification Suite | End-to-end and unit test verification in `aura-core` and `tools/ast-blast-radius` fulfilling all acceptance criteria | M4.3 | ORIGINAL_REQUEST §Acceptance Criteria |

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| M4.1 | Git Stubs Implementation | Complete `check_drift`, `sandbox_lint`, `open_pr`, and `apply_patch` in `aura-core/src/knowledge/git.rs` with full unit tests | none | DONE |
| M4.2 | AST Blast Radius MCP Wiring | Implement MCP protocol handling in `tools/ast-blast-radius/src/mcp.rs`, re-export in `lib.rs`, add tests in `tests/blast_radius_test.rs` | none | DONE |
| M4.3 | Final Acceptance Verification | Verify all acceptance criteria across `aura-core` and `tools/ast-blast-radius` with auditor and gate verification | M4.1, M4.2 | DONE |

## Interface Contracts

### GitManager API (`aura-core/src/knowledge/git.rs`)
```rust
pub struct GitManager {
    pub current_branch: String,
    pub tracked_files: Arc<Mutex<Vec<PathBuf>>>,
    pub merkle_tree: Arc<Mutex<Option<MerkleTree>>>,
    pub authored_files: Arc<Mutex<HashSet<PathBuf>>>,
}

impl GitManager {
    pub fn new(branch: impl Into<String>) -> Self;
    pub fn with_files(self, files: &[PathBuf]) -> Self;
    pub fn init_baseline(&self, files: &[PathBuf]);
    pub fn record_agent_change(&self, path: impl AsRef<Path>);
    pub fn check_drift(&self) -> Result<(), &'static str>;
    pub fn sandbox_lint(&self) -> Result<(), String>;
    pub fn open_pr(&self, title: &str, body: &str) -> Result<String, String>;
    pub fn apply_patch(&self, patch: &str) -> Result<(), String>;
}
```

### AST Blast Radius MCP Contract (`tools/ast-blast-radius/src/mcp.rs`)
```rust
pub fn handle_mcp_request(engine: &KnowledgeGraphEngine, request_json: &str) -> String;

// MCP Request format:
// {"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "ast-blast-radius", "arguments": {"symbol_id": "aura_core::runtime"}}}
// or {"target_symbol_id": "..."}

// MCP Response format:
// {"jsonrpc": "2.0", "id": 1, "result": {"content": [{"type": "text", "text": "{\"target_symbol_id\": ..., \"affected_count\": ..., \"risk_score\": ..., \"requires_escalation\": ...}"}]}}
```

## Code Layout
- `aura-core/src/knowledge/git.rs`: Git management implementation & tests (owned by M4.1 Worker)
- `tools/ast-blast-radius/src/mcp.rs`: MCP protocol implementation (owned by M4.2 Worker)
- `tools/ast-blast-radius/src/lib.rs`: Re-exports `mcp` module and `handle_mcp_request` (owned by M4.2 Worker)
- `tools/ast-blast-radius/tests/blast_radius_test.rs`: MCP unit & integration tests (owned by M4.2 Worker)
