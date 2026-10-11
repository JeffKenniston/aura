# Plan: M4 Repo Intelligence Completion

## Phase 0: Survey & Scoping
- Step 0.1: Spawn 3 parallel Explorers to investigate:
  - Explorer 1: `git.rs` stubs, `indexer.rs` (MerkleTree::update), BLAKE3 hashing, and `check_drift` requirements.
  - Explorer 2: Firecracker hypervisor module (`aura-core/src/hypervisor/firecracker.rs`), linter sandbox execution (`sandbox_lint`), ephemeral tunnel (`open_pr`), and `apply_patch` integration.
  - Explorer 3: `tools/ast-blast-radius` tool structure, MCP transport wiring, recursive CTE logic, input/output schemas, and test harnesses.
- Step 0.2: Merge findings into `PROJECT.md` with full feature inventory and interface contracts.

## Phase 1: Milestone M4.1 — `git.rs` Stubs Implementation
- Implement `check_drift`, `sandbox_lint`, `open_pr`, and `apply_patch`.
- Execute iteration loop: Explorer -> Worker -> Reviewers (2) -> Challengers (2) -> Forensic Auditor.
- Verify `aura-core` cargo tests for drift detection and patch application gating.

## Phase 2: Milestone M4.2 — `ast-blast-radius` MCP Wiring
- Implement MCP transport integration for `ast-blast-radius`.
- Execute iteration loop: Explorer -> Worker -> Reviewers (2) -> Challengers (2) -> Forensic Auditor.
- Verify `tools/ast-blast-radius` cargo tests with mock MCP requests and CTE blast radius responses.

## Phase 3: Acceptance & Dual-Track Verification
- Run complete test suite and acceptance criteria verification.
- Audit check and final status reporting to parent orchestrator.
