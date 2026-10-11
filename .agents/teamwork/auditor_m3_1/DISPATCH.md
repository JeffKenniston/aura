# Dispatch for Forensic Auditor M3

You are `auditor_m3_1` (role: Forensic Auditor, type: `teamwork_preview_auditor`).
Your assigned working directory is `/home/jeff/aura/.agents/teamwork/auditor_m3_1/`.

## Mandatory Reading:
- User Request: `/home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md` (R3)
- Master Baseline: `/home/jeff/aura/M0_M3_Audit_Report.md` (§ BUS-003, EXE-VM-002, EXE-VM-011)
- Scope: `/home/jeff/aura/.agents/teamwork/orchestrator_7/PROJECT.md`
- Repository Constitution: `/home/jeff/aura/AGENTS.md` (Strict zero mock/stub policy)
- Worker M3 Handoff: `/home/jeff/aura/.agents/teamwork/worker_m3_1/handoff.md`

## Milestone 3 Files to Inspect & Audit:
1. `aura-core/src/bus/mod.rs` (BUS-003)
2. `aura-core/src/hypervisor/firecracker.rs` (EXE-VM-002, EXE-VM-011)
3. `aura-core/src/hypervisor/mod.rs`
4. `aura-core/Cargo.toml`

## Mandatory Tasks:
1. Run all forensic integrity checks on the M3 implementation files.
2. Verify that there are ABSOLUTELY NO mocks, stubs, TODOs, or fake test results in production code (AGENTS.md).
3. Verify that `bus/mod.rs` implements genuine dynamic SVID extraction (4 tiers), Zero-Trust capability validation (`validate_svid`), rate limiting (`check_and_consume`), and real execution dispatch via `Registry` / Cranelift JIT engine and Firecracker microVMs (not static string formatting).
4. Verify that `hypervisor/firecracker.rs` implements authentic `validate_kvm_support()` and typed `KvmError` checking `/dev/kvm`, and supports `AURA_SKIP_KVM_VALIDATION`.
5. Verify that Firecracker VM config enforces `"is_read_only": true` on the rootfs, `ro root=/dev/vda` in boot args, and cleans up ephemeral scratch disks on teardown.
6. Execute `cargo test -p aura-core --lib hypervisor::firecracker::tests`
7. Execute `cargo test -p aura-core --lib bus::tests`
8. Execute full `cargo test -p aura-core` (all 128 tests must pass).
9. Write your comprehensive audit report in `/home/jeff/aura/.agents/teamwork/auditor_m3_1/handoff.md`.
10. Conclude with a clear binary verdict: **CLEAN** or **INTEGRITY VIOLATION**.
11. Send a message to orchestrator with your verdict and findings.

## 2026-10-11T03:59:36Z
[Message] timestamp=2026-10-11T03:59:36Z sender=b0e73c5b-179f-432b-9bbd-ebac6eea2996 priority=MESSAGE_PRIORITY_HIGH content=You are auditor_m3_1 (role: Forensic Auditor, type: teamwork_preview_auditor).
Your working directory is `/home/jeff/aura/.agents/teamwork/auditor_m3_1/`.
Read your task assignment in `/home/jeff/aura/.agents/teamwork/auditor_m3_1/DISPATCH.md`.
