# Task Assignment: Reviewer 2 (M4 Full Scope Review)

- Working Directory: /home/jeff/aura/.agents/teamwork/reviewer_2/
- Parent Conversation ID: 966c9504-d054-4c12-b035-b5bb1de49d34
- User Request: /home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md
- Project Spec: /home/jeff/aura/PROJECT.md
- Worker Handoffs:
  - /home/jeff/aura/.agents/teamwork/worker_m4_1/handoff.md
  - /home/jeff/aura/.agents/teamwork/worker_m4_2/handoff.md

## Scope of Review
Perform an independent, adversarial and thorough review of both M4 implementations:
1. `aura-core/src/knowledge/git.rs`:
   - Verify `check_drift` thread-safety, BLAKE3 Merkle tree updates, conflict detection.
   - Verify `sandbox_lint` Firecracker microVM execution and R3 constraint compliance.
   - Verify `open_pr` ephemeral credential lifecycle (minting & revocation).
   - Verify `apply_patch` strict sequential halting on drift and lint errors.
2. `tools/ast-blast-radius`:
   - Verify MCP JSON-RPC 2.0 message parsing, error codes, and response structure.
   - Verify compatibility with both `symbol_id` and `target_symbol_id`.
   - Verify recursive CTE blast radius calculation and threshold escalation logic.

## Verification
Run tests:
```bash
cargo test --package aura-core
cargo test --manifest-path tools/Cargo.toml -p ast-blast-radius
```

Check:
- Correctness, completeness, edge case handling, zero stubs/mocks/TODOs.
- State your verdict clearly: **APPROVE** or **REQUEST_CHANGES**.

Write your report in `/home/jeff/aura/.agents/teamwork/reviewer_2/handoff.md` and message the orchestrator.
