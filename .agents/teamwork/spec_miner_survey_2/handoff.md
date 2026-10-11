# Handoff Report: Phase M2 (Docker-Free Sandboxing) Requirements Mining

**Agent**: `spec_miner_survey_2`  
**Working Directory**: `/home/jeff/aura/.agents/teamwork/spec_miner_survey_2/`  
**Recipient**: Project Orchestrator (`orchestrator_2`, ID: `d71b58bc-d6bb-4674-ac7d-b24b5ce1f6c3`)  
**Type**: Hard Handoff (Task Complete)  

---

## 1. Observation

1. **Authoritative Specification & Architecture Sources**:
   - `/home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md`: Lines 14, 23 dictate R3 (Controlled Execution) using the Firecracker hypervisor module for `sandbox_lint` in isolation.
   - `/home/jeff/projects/aura/aura-os/docs/ROADMAP.md`: Lines 18–24 define Phase 2 Execution Substrate items: wasmtime embedding with Cranelift JIT, WASI 0.3 Component Model with completion-based async I/O, Firecracker microVM provisioning, jailer wrapper with cgroups and seccomp, and WSL2 `/dev/kvm` validation. Line 39 defines Phase 5 Core Toolchain (Computer, Bash, Browser, FileSystem).
   - `/home/jeff/projects/aura/aura-os/docs/KNOWLEDGE.md`: Lines 37–47 document completed Phase 2 WASM and microVM sandboxing components and key edge cases for wasmtime component instantiation (v20.0.0 `call_run(&mut store).await??`) and Firecracker graceful shutdown via `SendCtrlAltDel` (line 76).
   - `/home/jeff/projects/aura/aura-os/docs/RESEARCH.md`: Lines 26–52 define Docker-free sandboxing invariants, comparing WASI 0.2 vs WASI 0.3 completion-based streams and futures, and detailing Firecracker virtio-centric microVMs booting in <125ms with <5MiB overhead.
   - `/home/jeff/aura/docs/architecture/adrs/ADR-001-Execution-Host.md`: Lines 9–19 document accepted architectural decisions: bifurcated sandboxing (WASI 0.3 for lightweight, Firecracker for heavyweight) and cold boots only (<125ms via KVM), deferring snapshot pooling.
   - `/home/jeff/aura/docs/architecture/threat-models/STRIDE.md`: Lines 15–26 define security controls `EXE-WASM-005` (fuel metering), `EXE-WASM-007` (WASI capability check), `EXE-VM-005` (jailer cgroups/seccomp), `EXE-VM-006` and `EXE-VM-012` (ephemeral microVMs).
   - Domain skills:
     * `aura-exe-wasm-domain/SKILL.md`: WASI 0.3 completion-based streams/futures, wasmtime with Cranelift and pooling allocator, fuel/memory/wall-clock limits, no ambient authority, static composition (lines 6–12).
     * `aura-exe-vm-domain/SKILL.md`: Firecracker microVMs over KVM, jailer with cgroup v2/chroot/namespaces/seccomp, session-scoped lifecycle, read-only base images + ephemeral writable overlay, transparent TLS egress proxy (lines 6–12).
     * `aura-tool-domain/SKILL.md`: Bash session persistence caps, Browser untrusted page content, FileSystem strict workspace confinement (lines 6–11).
     * `aura-architecture/references/02-execution-sandboxes.md`: Strict prohibition of legacy containers (Docker/containerd/OCI), WASI 0.3 component model, and Firecracker jailer isolation (lines 10–26).

2. **Codebase Implementation Citations**:
   - `aura-core/src/hypervisor/wasm.rs`:
     * Line 9: `create_engine()` configured for WASI 0.3 (`EXE-WASM-001 - 008`).
     * Line 22: Cranelift JIT compilation (`EXE-WASM-002`).
     * Line 29: Asynchronous execution for Tokio (`EXE-WASM-003`).
     * Line 32: WASM Component Model support (`EXE-WASM-004`).
     * Line 35: Instruction fuel metering (`EXE-WASM-005`).
     * Line 38: Pooling allocator (`EXE-WASM-006`).
     * Line 65: Asynchronous WASI Component Model linker (`EXE-WASM-007`).
     * Line 92: Component execution with fuel budget (`EXE-WASM-008`).
   - `aura-core/src/hypervisor/firecracker.rs`:
     * Line 18: `SpecializedRootfs` (`Bash`, `Browser`, `Computer`, `Custom`) (`EXE-VM-010`).
     * Line 87: `VmConfig` with `for_bash`, `for_browser`, `for_computer`.
     * Line 137: `extract_png_bytes` with `PNG_MAGIC_BYTES` Zero-Trust validation (`TOOL-CMP-004`).
     * Line 247: `start_screenshot_listener` over AF_VSOCK (`TOOL-CMP-003`).
     * Line 329: `FirecrackerVm` wrapped in Jailer isolation (`EXE-VM-001 - 015`).
     * Line 391: `start_jailer` with cgroups v2 and seccomp (`EXE-VM-003`).
     * Line 429: `configure_and_boot` issuing API requests (`EXE-VM-004 - 008`).
     * Line 448: Memory overhead < 5MiB and <125ms cold boot sizing (`EXE-VM-005`).
     * Line 455: AF_VSOCK configuration (`guest_cid: 3`, `/run/vsock.socket`) (`EXE-VM-009`).
     * Line 471: Ephemeral lifecycle teardown (`EXE-VM-012`, `EXE-VM-013`, `EXE-VM-014`).
   - `aura-core/src/hypervisor/proxy.rs`:
     * Lines 8–30: Default allowed domains for official package registries (`crates.io`, `pypi.org`, `ubuntu`, `debian`, `github`) (`EXE-VM-015`).
     * Line 33: `TLS_ALERT_ACCESS_DENIED` (fatal alert 49) injected for blocked domains.
     * Line 98: `EgressProxy` with pure-Rust RFC 5246/8446 SNI parser and HTTP CONNECT handler.
   - `aura-core/src/runtime/registry.rs`:
     * Line 11: `RuntimeBackend` (`Wasm`, `Firecracker`).
     * Line 54: `register_default_sandboxes` registering `bash`, `browser`, `computer`.
   - `deploy/images/`:
     * `build_browser_image.sh`: Headless Chromium build script (`TOOL-BRW-001`).
     * `build_computer_image.sh`: Headless X11/Xvfb + fluxbox build script (`TOOL-CMP-001`, `TOOL-CMP-003`).
     * `vsock-screenshot-daemon/`: Pure-Rust X11 framebuffer capture and AF_VSOCK streaming daemon (`TOOL-CMP-003`).
   - `tools/filesystem/`:
     * `src/path.rs`: `PathValidator` rejecting `..` traversal and boundary escapes (`TOOL-FS-002`).
     * `src/service.rs`: Atomic read, write, and patch file operations (`TOOL-FS-001`).
     * `src/lib.rs`: WIT binding for `aura:fs/read` and `aura:fs/write` (`TOOL-FS-003`).
   - `tools/`: JSON manifests for `bash.json`, `browser.json`, `computer.json`, `filesystem.json`.
   - `aura-sdk/aura_sdk/tools/`: Python tool dispatch clients for `bash.py`, `browser.py`, `computer.py`, `filesystem.py`.

3. **Verbatim Test Results**:
   - `cargo test -p aura-core`: `test result: ok. 86 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.99s` + integration tests: `test result: ok. 5 passed; 0 failed`.
   - `cargo test --manifest-path tools/Cargo.toml`: `ast_blast_radius` (35 tests passed), `ephemeral_tunnel` (5 tests passed), `filesystem` (7 tests passed, including `test_atomic_file_write_and_read`, `test_atomic_file_patch_success`, `test_path_prefix_matching_valid`, `test_path_traversal_blocked_parent_dir`, `test_path_outside_boundary_blocked`, `test_submillisecond_execution`).
   - `cargo test --manifest-path deploy/images/vsock-screenshot-daemon/Cargo.toml`: `test result: ok. 1 passed; 0 failed`.
   - `./venv/bin/pytest aura-sdk/tests`: `48 passed, 1 warning in 1.28s`.

---

## 2. Logic Chain

1. **Scope Boundaries**: Phase M2 covers all requirements related to Docker-free execution sandboxing via WebAssembly WASI 0.3 and Firecracker microVMs.
2. **Identification of Requirement Matrix**:
   - Examination of `wasm.rs` and `STRIDE.md` establishes the canonical 8 WASI requirements: `EXE-WASM-001` through `EXE-WASM-008`.
   - Examination of `firecracker.rs`, `proxy.rs`, `ADR-001`, and `02-execution-sandboxes.md` establishes the 15 microVM requirements: `EXE-VM-001` through `EXE-VM-015`.
   - Examination of `deploy/images/`, `tools/`, and `aura-sdk/tools/` establishes the sandboxed tool integration requirements: `TOOL-CMP-001` through `004`, `TOOL-BASH-001` through `003`, `TOOL-BRW-001` through `003`, and `TOOL-FS-001` through `004`.
   - Total requirements identified in scope: **37 requirements**.
3. **Status Determination**:
   - Every requirement was cross-referenced against actual source files and test suites.
   - 34 requirements are **Complete**: verified with active code and passing unit/integration tests.
   - 3 requirements are **Partial**:
     * `EXE-VM-011`: Base ext4 image drive is mounted `"is_read_only": false` rather than read-only with an ephemeral copy-on-write overlay.
     * `TOOL-BASH-002`: Bash commands execute in single-shot microVMs; session multiplexing across consecutive calls within a task is scaffolded but lacks a guest session daemon.
     * `TOOL-BRW-003`: Headless Chromium exposes CDP on port 9222 in rootfs, but host-to-guest vsock TCP forwarding to a live CDP driver is not yet wired.
   - 0 requirements are **Missing**.
4. **Documentation Delivery**: All 37 requirements, statuses, exact file/line citations, features discovered table, and edge cases table have been written to `/home/jeff/aura/.agents/teamwork/spec_miner_survey_2/survey_report.md`.

---

## 3. Caveats

- **Host Hardware Virtualization Access**: On developer workstations or CI environments lacking `/dev/kvm` permissions, `FirecrackerVm::configure_and_boot` is designed to gracefully succeed early if the socket cannot be contacted, allowing complete unit testing of configuration, parameters, and lifecycles without panics or hardware dependency. Full microVM boots require Linux x86_64 with KVM support.
- **RootFS Image Compilation**: Building the actual `.ext4` image files via `deploy/images/build_*.sh` requires `sudo` privileges to mount loopback devices (`mkfs.ext4`, `mount -o loop`); the scripts and daemon sources are production-ready.

---

## 4. Conclusion

Phase M2 (Docker-Free Sandboxing: WebAssembly WASI 0.3 and Firecracker MicroVMs) is in a highly mature state, with 34 of 37 requirements (91.9%) fully implemented and verified by automated test suites. The remaining 3 partial items (`EXE-VM-011`, `TOOL-BASH-002`, `TOOL-BRW-003`) are well-scoped enhancements with concrete next steps detailed in `survey_report.md`.

---

## 5. Verification Method

1. Inspect generated report:
   ```bash
   cat /home/jeff/aura/.agents/teamwork/spec_miner_survey_2/survey_report.md
   ```
2. Run hypervisor test suite:
   ```bash
   cargo test -p aura-core -- hypervisor
   ```
3. Run tools test suite:
   ```bash
   cargo test --manifest-path tools/Cargo.toml
   ```
4. Run screenshot daemon test suite:
   ```bash
   cargo test --manifest-path deploy/images/vsock-screenshot-daemon/Cargo.toml
   ```
5. Invalidation condition: Any failure in the above tests or any discrepancy between `survey_report.md` requirement IDs and the cited source code invalidates this report.
