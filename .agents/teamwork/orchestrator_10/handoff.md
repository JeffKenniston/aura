# Orchestrator Soft Handoff — Phase M5 (Hardening)

**From:** `orchestrator_10` (Gen 1)  
**To:** Successor Orchestrator (`orchestrator_11`, Gen 2)  
**Working Directory:** `/home/jeff/aura/.agents/teamwork/orchestrator_10`  
**Parent Conversation ID:** `c605b745-0ee6-4f23-9cfa-5af1c0a06c07`  
**Date:** 2026-10-11T18:26:00Z  
**Type:** Soft Handoff (Spawn threshold reached: 18 / 16)  

---

## 1. Milestone State

| # | Name | Scope | Status | Notes |
|---|------|-------|:------:|-------|
| **Survey** | Codebase & Requirement Survey | R1, R2, R3, R4 | **DONE** | 3 survey reports in `teamwork_preview_spec_miner_survey_1`, `explorer_survey_2`, `explorer_survey_3`. |
| **PROJECT.md** | Global Architecture & Feature Inventory | 23 Features (100% assigned) | **DONE** | Created at `/home/jeff/aura/PROJECT.md`. |
| **TEST_INFRA.md** | 4-Tier Opaque-Box Test Architecture | Tiers 1-4 Specification | **DONE** | Created at `/home/jeff/aura/TEST_INFRA.md`. |
| **E2E Track** | Comprehensive Opaque-Box E2E Suite | 170 tests across 15 features | **DONE** | `aura-core/tests/m5_e2e_opaque_test.rs` authored; verified passing (170/170 pass in 0.12s); published `TEST_READY.md`. |
| **M1** | Ephemeral Credential Lifecycle & VCS Integration | Features 1-6 | **IN_PROGRESS (It2)** | It1 Worker passed 325/325 tests in aura-core and 11/11 in tunnel. Gate 1 It1 rejected due to `TokenMetadata` missing custom Display/Debug and 8-char boundary leakage. **It2 exploration completed**: drop-in patches ready in `explorer_m1_it2_1`, `explorer_m1_it2_2`, and `explorer_m1_it2_3`. |
| **M2** | Sandboxed GUI Automation & Visual Redaction | Features 7-12 | **PLANNED** | Surveyed by `explorer_survey_2`. Ready for worker dispatch. |
| **M3** | Microkernel Zero-Trust Security & Storage Encryption | Features 13-17 | **PLANNED** | Surveyed by `explorer_survey_3`. Ready for worker dispatch. |
| **M4** | SPIRE Identity Federation & Transport Fallback | Features 18-21 | **PLANNED** | Surveyed by `explorer_survey_3`. Ready for worker dispatch. |
| **M5_FINAL** | 100% E2E Pass & Adversarial Coverage Hardening | Phase 1 (E2E) & Phase 2 (Tier 5) | **PLANNED** | Requires M1-M4 completion. |

---

## 2. Active Subagents
All 18 spawned subagents have delivered their handoff reports and are in `idle` state. There are zero pending subagents.

---

## 3. Pending Decisions & Immediate Context
Milestone 1 Iteration 2 is fully explored and ready for worker implementation:
1. `tools/ephemeral-tunnel/src/credentials.rs`:
   - Combine fixes from `explorer_m1_it2_1` (`credentials_token_metadata.patch` / `proposed_credentials.rs`) and `explorer_m1_it2_2` (`credentials_boundary.patch` / `proposed_credentials.rs`).
   - Implement custom `fmt::Display` and `fmt::Debug` on `TokenMetadata` that mask `self.token` using `[REDACTED-TOKEN-...]`.
   - Update suffix extraction so that tokens with `chars.len() <= 8` clamp to `...` (`[REDACTED-TOKEN-...]`), preventing 100% secret byte disclosure.
2. `tools/ephemeral-tunnel/tests/tunnel_test.rs`:
   - Apply `explorer_m1_it2_3` patch (`/home/jeff/aura/.agents/teamwork/explorer_m1_it2_3/tunnel_test_companion.patch`) adding tests for `TokenMetadata` formatting and boundary tests, reconciling line 176.
3. Once applied, run Gate 1 Iteration 2 verification:
   - Worker runs `cargo test --manifest-path tools/ephemeral-tunnel/Cargo.toml` and `cargo test -p aura-core --jobs 1`.
   - Dispatch 2 Reviewers, 2 Challengers, 1 Forensic Auditor (`teamwork_preview_auditor`).
   - Once Gate 1 PASS is recorded in `GATE_STATUS.md`, proceed to Milestone 2.

---

## 4. Remaining Work (Concrete Next Steps for Successor)

1. **Step 1: Milestone 1 Iteration 2 Worker Implementation & Gate**:
   - Spawn `worker_m1_it2_1` (or apply patches from `explorer_m1_it2_1`, `explorer_m1_it2_2`, `explorer_m1_it2_3`).
   - Verify `cargo test --manifest-path tools/ephemeral-tunnel/Cargo.toml` and `cargo test -p aura-core --jobs 1`.
   - Verify Gate 1 Iteration 2 with Reviewers, Challengers, and Forensic Auditor.
   - Record `Gate Result: PASS` in `GATE_STATUS.md` and mark M1 `DONE` in `progress.md`.

2. **Step 2: Milestone 2 (Sandboxed GUI Automation & Visual Redaction)**:
   - Target files:
     - `deploy/images/vsock-screenshot-daemon/src/capture.rs`: fix `plane_mask` depth calculation (`(1u32 << screen.root_depth) - 1`).
     - `deploy/images/vsock-screenshot-daemon/src/input.rs` & `server.rs`: add JSON-RPC 2.0 synthetic input handling (`click`, `move`, `drag`, `type`, `key`, `screenshot`).
     - `aura-core/src/hypervisor/firecracker.rs`: reconcile vsock port 5254.
     - `aura-sdk/aura_sdk/tools/computer.py`: implement async `ComputerTool` (`click`, `move`, `drag`, `type`, `key`, `screenshot`, `capture_redacted_screenshot` with pure Python PNG scanline redaction).
     - `aura-sdk/tests/test_computer_tool.py`: 100% passing tests.
   - Execute iteration loop (Worker -> Reviewers -> Challengers -> Auditor -> Gate 2).

3. **Step 3: Milestone 3 (Microkernel Zero-Trust Security & Storage Encryption)**:
   - Target files:
     - `aura-core/src/hypervisor/firecracker.rs`: add `VsockStreamConfig { max_buffer_bytes }` and instant stream truncation on limit violation.
     - `aura-core/src/hypervisor/proxy.rs` & `mod.rs`: re-export `EgressProxy` and wire transparent TLS proxy issuing Alert 49 on unauthorized domains.
     - `aura-core/src/storage/`: implement `EncryptedDatabase` (AES-256-GCM container over `rusqlite` using BLAKE3 KDF key from `Svid`) and `TaskLedger`. Assert raw disk inspection confirms ciphertext with zero plaintext strings.
   - Execute iteration loop (Worker -> Reviewers -> Challengers -> Auditor -> Gate 3).

4. **Step 4: Milestone 4 (SPIRE Identity Federation & Transport Fallback Resilience)**:
   - Target files:
     - `aura-core/src/identity/`: implement `SpireFederationMapping`, `OidcProviderConfig`, `verify_oidc_token`.
     - `aura-core/src/transport/`: wire `McpServer` to HTTP/2 TCP listener in `http3.rs`; implement `ResilientMcpTransportClient` detecting UDP/QUIC failure within 1 RTT (<100ms) and replaying in-flight calls on TCP.
   - Execute iteration loop (Worker -> Reviewers -> Challengers -> Auditor -> Gate 4).

5. **Step 5: Final Milestone (100% E2E Pass & Adversarial Hardening)**:
   - Phase 1: Run full workspace test gates:
     - `cargo test -p aura-core --jobs 1` (must pass 100%)
     - `cargo test --manifest-path tools/ephemeral-tunnel/Cargo.toml`
     - `cargo test --manifest-path deploy/images/vsock-screenshot-daemon/Cargo.toml`
     - `/home/jeff/aura/aura-sdk/.venv/bin/pytest aura-sdk/tests`
     - `/home/jeff/aura/aura-sdk/.venv/bin/ruff check aura-sdk/aura_sdk aura-sdk/tests`
     - `git status` clean working tree.
   - Phase 2: Tier 5 Adversarial Coverage Hardening (Challenger gap analysis -> Worker fixes -> Auditor).
   - Deliver victory handoff to parent `c605b745-0ee6-4f23-9cfa-5af1c0a06c07`.

---

## 5. Key Artifacts
- `/home/jeff/aura/PROJECT.md`
- `/home/jeff/aura/TEST_INFRA.md`
- `/home/jeff/aura/TEST_READY.md`
- `/home/jeff/aura/aura-core/tests/m5_e2e_opaque_test.rs`
- `/home/jeff/aura/.agents/teamwork/orchestrator_10/GATE_STATUS.md`
- `/home/jeff/aura/.agents/teamwork/orchestrator_10/progress.md`
- `/home/jeff/aura/.agents/teamwork/explorer_m1_it2_1/strategy_report.md`
- `/home/jeff/aura/.agents/teamwork/explorer_m1_it2_2/strategy_report.md`
- `/home/jeff/aura/.agents/teamwork/explorer_m1_it2_3/strategy_report.md`
