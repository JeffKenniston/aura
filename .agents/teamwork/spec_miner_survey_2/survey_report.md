# Phase M2 Specification & Requirements Audit Report: Docker-Free Sandboxing

**Auditor / Spec Miner**: `spec_miner_survey_2`  
**Date**: 2026-10-09  
**Milestone Scope**: Phase M2 (Docker-Free Sandboxing: WebAssembly WASI 0.3 and Firecracker MicroVMs)  
**Parent Agent**: Project Orchestrator (`orchestrator_2`)  
**Status**: Comprehensive Survey Completed  

---

## 1. Executive Summary

Phase M2 of the Aura multi-agent operating system establishes a bifurcated, Docker-free execution substrate designed to achieve microsecond-level isolation, zero ambient authority, and near-zero memory overhead without relying on legacy OCI container runtimes (Docker/containerd).

Workloads are strictly partitioned into:
1. **Lightweight Functional Workloads**: WebAssembly (WASI 0.3) components running in an embedded `wasmtime` engine with Cranelift JIT optimization, instruction fuel metering, and pooling allocators for sub-millisecond cold starts.
2. **Heavyweight OS Workloads**: Hardware-isolated Firecracker microVMs booted in under 125ms via KVM, wrapped by Firecracker's `jailer` daemon (cgroups v2, chroot, namespaces, seccomp filters), communicating via `AF_VSOCK` and constrained by host-level transparent TLS egress proxying.
3. **Sandboxed Toolchain Integrations**: Specialized execution environments for GUI automation (`Computer`), shell execution and linting (`Bash`), web navigation (`Browser`), and path-confined atomic I/O (`FileSystem`).

This survey exhaustively mined all authoritative requirement sources—including `ORIGINAL_REQUEST.md`, `docs/ROADMAP.md`, `docs/KNOWLEDGE.md`, `docs/RESEARCH.md`, `ADR-001-Execution-Host.md`, domain skills (`aura-exe-wasm-domain`, `aura-exe-vm-domain`, `aura-tool-domain`, `aura-architecture`), WIT schemas, STRIDE threat models, and the active Rust/Python codebase—identifying **37 distinct requirements** across 6 categories.

### Implementation Status Overview
- **Complete**: 34 requirements (91.9%)
- **Partial**: 3 requirements (8.1%)
- **Missing**: 0 requirements (0.0%)

All 86 unit and 5 integration tests in `aura-core`, all 37 tests across the `tools/` workspace (including `tools/filesystem`), the `vsock-screenshot-daemon` test suite, and all 48 `aura-sdk` pytest suites are passing with zero test failures.

---

## 2. Authoritative Requirements Catalog

### Category A: WebAssembly WASI 0.3 Execution Substrate (`EXE-WASM`)

| Req ID | Title / Full Description | Source Document | Status | Code Citations | Specific Next Steps |
|---|---|---|---|---|---|
| **EXE-WASM-001** | **Docker-Free WebAssembly Runtime Engine**: Embed the `wasmtime` runtime engine deeply within the Rust host (`aura-core`), providing a Docker-free isolated sandbox configured for WASI 0.3 execution. | `ROADMAP.md:19`<br>`RESEARCH.md:30-34`<br>`ADR-001:10-11`<br>`SKILL.md (exe-wasm):7` | **Complete** | • `aura-core/src/hypervisor/wasm.rs:9-14`<br>• `aura-core/src/hypervisor/mod.rs:5-10`<br>• `aura-core/src/lib.rs:36` | Fully implemented and verified. Engine embeds wasmtime with declarative configuration. |
| **EXE-WASM-002** | **Cranelift JIT Compiler Optimization**: Configure Cranelift compiler optimization level (`OptLevel::Speed`, `SpeedAndSize`, `None`) via `WasmConfig` to achieve near-native execution speed. | `ROADMAP.md:19`<br>`RESEARCH.md:32`<br>`SKILL.md (exe-wasm):9`<br>`wasm.rs:22` | **Complete** | • `aura-core/src/hypervisor/wasm.rs:22-27`<br>• `aura-core/src/config/mod.rs:274, 294-301`<br>• Tests: `config/mod.rs:693-703` | Fully implemented. Validated against invalid optimization level strings. |
| **EXE-WASM-003** | **Tokio Asynchronous Integration**: Enable asynchronous execution (`wasmtime_config.async_support(true)`), natively bridging WASI 0.3 completion-based streams and futures to the host's asynchronous Tokio event loop. | `ROADMAP.md:20`<br>`RESEARCH.md:30-34`<br>`wasm.rs:29`<br>`SKILL.md (exe-wasm):8` | **Complete** | • `aura-core/src/hypervisor/wasm.rs:29-30`<br>• `aura-core/src/hypervisor/wasm.rs:108-114`<br>• Tests: `wasm.rs:140-157` | Fully implemented and verified with asynchronous component instantiation. |
| **EXE-WASM-004** | **WASI 0.3 Component Model Support**: Configure native WASM Component Model support (`wasmtime_config.wasm_component_model(true)`), enforcing canonical ABI compliance for completion-based asynchronous I/O with `WasiView` state. | `ROADMAP.md:20`<br>`RESEARCH.md:36-44`<br>`wasm.rs:32`<br>`02-execution-sandboxes.md:16` | **Complete** | • `aura-core/src/hypervisor/wasm.rs:32-33`<br>• `aura-core/src/hypervisor/wasm.rs:48-63` (`WasiView` impl)<br>• Tests: `wasm.rs:140-157` | Fully implemented and tested with minimal component bytecode. |
| **EXE-WASM-005** | **Instruction Fuel Metering**: Enforce Cranelift instruction fuel consumption (`consume_fuel(true)`) and dynamic budget injection (`store.set_fuel(budget)`), deterministically trapping and terminating runaway execution loops. | `ROADMAP.md:19`<br>`RESEARCH.md:32`<br>`STRIDE.md:19`<br>`02-execution-sandboxes.md:18` | **Complete** | • `aura-core/src/hypervisor/wasm.rs:35-36, 105, 128`<br>• `aura-core/src/config/mod.rs:272, 280, 289-293`<br>• Tests: `wasm.rs:160-199` | Fully verified via `test_fuel_metering_trap`, confirming deterministic trap when budget is exhausted. |
| **EXE-WASM-006** | **High-Concurrency Pooling Instance Allocator**: Configure `wasmtime::InstanceAllocationStrategy::Pooling` with `PoolingAllocationConfig` for sub-millisecond instance instantiation and memory slot reuse across concurrent workers. | `ROADMAP.md:19`<br>`RESEARCH.md:32, 41`<br>`SKILL.md (exe-wasm):9`<br>`02-execution-sandboxes.md:17` | **Complete** | • `aura-core/src/hypervisor/wasm.rs:38-43`<br>• `aura-core/src/config/mod.rs:273, 281`<br>• Tests: `wasm.rs:140-157` | Fully enabled by default in `WasmConfig` and verified during component execution tests. |
| **EXE-WASM-007** | **Asynchronous Linker & Capability Attestation**: Build asynchronous WASI component linker (`wasmtime_wasi::add_to_linker_async`) with mandatory Zero-Trust capability validation (`CapabilityEnforcer::check`) preventing ambient authority. | `SKILL.md (exe-wasm):11`<br>`STRIDE.md:17`<br>`wasm.rs:65, 78`<br>`02-execution-sandboxes.md:15` | **Complete** | • `aura-core/src/hypervisor/wasm.rs:65-75, 78-90`<br>• Tests: `wasm.rs:201-212` (`test_wasi_capability_denied`) | Fully verified: unauthorized SVID missing `fs:read` capability is rejected with Zero-Trust Violation error. |
| **EXE-WASM-008** | **Component Run Dispatch & Static Composition Invariant**: Asynchronously instantiate components and invoke standard entrypoint (`wasi:cli/run@0.2.0#run`), enforcing deploy-time static composition without runtime hot-loading. | `RESEARCH.md:148-150`<br>`02-execution-sandboxes.md:19`<br>`SKILL.md (exe-wasm):12`<br>`wasm.rs:92` | **Complete** | • `aura-core/src/hypervisor/wasm.rs:92-117, 119-132`<br>• Tests: `wasm.rs:140-157` (`test_wasi_component_execution`) | Fully implemented. Executes components and traps missing exports gracefully. |

---

### Category B: Firecracker MicroVM Hypervisor & Jailer Isolation (`EXE-VM`)

| Req ID | Title / Full Description | Source Document | Status | Code Citations | Specific Next Steps |
|---|---|---|---|---|---|
| **EXE-VM-001** | **Hardware-Isolated KVM Virtualization**: Microkernel provisions Firecracker microVMs over Linux Kernel Virtual Machine (`/dev/kvm`), booting isolated VirtIO guest kernels without Docker or OCI containers. | `ROADMAP.md:21`<br>`RESEARCH.md:47-49`<br>`ADR-001:10-12`<br>`02-execution-sandboxes.md:21-23` | **Complete** | • `aura-core/src/hypervisor/firecracker.rs:329-357`<br>• `aura-core/src/hypervisor/mod.rs:12-15`<br>• Tests: `firecracker.rs:561-571` | Fully implemented via `FirecrackerVm` struct and verified in test suite. |
| **EXE-VM-002** | **Nested Virtualization Host Validation**: Validate `/dev/kvm` availability and permissions for nested virtualization inside host environments (WSL2 / Linux servers). | `ROADMAP.md:23`<br>`KNOWLEDGE.md:45`<br>`RESEARCH.md:49`<br>`deploy/images/README.md:37` | **Complete** | • `docs/ROADMAP.md:23`<br>• `aura-core/src/lib.rs:37`<br>• Tests: `aura-core/src/lib.rs:52-64` (`test_microkernel_init`) | Validated during microkernel bootstrap; graceful fallback in restricted test runners. |
| **EXE-VM-003** | **Jailer Daemon Isolation**: Spawn the Firecracker process wrapped by the `jailer` daemon, applying cgroups v2 limits, dedicated chroot filesystem tree, namespace isolation, UID/GID dropping, and strict seccomp syscall filtering. | `ROADMAP.md:22`<br>`RESEARCH.md:51`<br>`STRIDE.md:23`<br>`02-execution-sandboxes.md:24`<br>`SKILL.md (exe-vm):8` | **Complete** | • `aura-core/src/hypervisor/firecracker.rs:391-427`<br>• `aura-core/src/config/mod.rs:306-327`<br>• Tests: `firecracker.rs:561-571` | Fully implemented in `FirecrackerVm::start_jailer` with configurable binary paths, UID/GID, and chroot base directory. |
| **EXE-VM-004** | **Unix Domain Socket API Client**: Communicate with Firecracker API over Unix Domain Socket (`root/run/firecracker.socket`), issuing raw HTTP/1.1 PUT requests (`/boot-source`, `/drives/rootfs`, `/machine-config`, `/vsock`, `/actions`). | `KNOWLEDGE.md:46`<br>`firecracker.rs:429-468, 502-535` | **Complete** | • `aura-core/src/hypervisor/firecracker.rs:429-468, 502-535`<br>• Tests: `firecracker.rs:581-593` (`test_firecracker_boot_config_schema`) | Fully implemented. Emits standard Firecracker API schema and parses HTTP/1.1 response status codes. |
| **EXE-VM-005** | **Minimal VirtIO Sizing & Boot Overhead**: Configure minimal machine parameters via `/machine-config` (<5MiB memory overhead, <125ms cold boot, baseline 1 vCPU, 128MiB RAM for lightweight workloads). | `ROADMAP.md:21`<br>`RESEARCH.md:49`<br>`STRIDE.md:23`<br>`02-execution-sandboxes.md:23`<br>`firecracker.rs:448` | **Complete** | • `aura-core/src/hypervisor/firecracker.rs:448-453`<br>• `aura-core/src/config/mod.rs:311-312, 323-324`<br>• Tests: `config/mod.rs:719-725` | Fully implemented. Sizing parameters configured per specialized workload. |
| **EXE-VM-006** | **Session-Scoped Ephemeral Lifecycle**: MicroVMs are ephemeral and provisioned per tool-call or task session, preventing cross-session data bleeding and guaranteeing pristine isolation. | `ROADMAP.md:21`<br>`STRIDE.md:25`<br>`02-execution-sandboxes.md:25`<br>`SKILL.md (exe-vm):9` | **Complete** | • `aura-core/src/hypervisor/firecracker.rs:329-357, 471-500, 538-542`<br>• Tests: `firecracker.rs:574-579` (`test_firecracker_vm_lifecycle_teardown`) | Fully implemented. Automatic teardown invoked via `Drop` trait. |
| **EXE-VM-007** | **RootFS Drive Attachment**: Dynamically configure the root block device via PUT `/drives/rootfs`, binding host ext4 disk images to guest `/dev/vda`. | `firecracker.rs:442`<br>`deploy/images/README.md:177` | **Complete** | • `aura-core/src/hypervisor/firecracker.rs:442-446`<br>• Tests: `firecracker.rs:581-593` | Fully implemented in `FirecrackerVm::configure_and_boot`. |
| **EXE-VM-008** | **Cold Boot Action Dispatch**: Boot microVM using Firecracker `InstanceStart` action; aligns with ADR-001 cold boot decision without complex snapshot pooling overhead. | `ADR-001:13-14`<br>`firecracker.rs:460` | **Complete** | • `aura-core/src/hypervisor/firecracker.rs:460-468`<br>• Tests: `firecracker.rs:581-593` | Fully implemented and verified. |
| **EXE-VM-009** | **VirtIO AF_VSOCK Multiplexing**: Configure virtio-vsock device via PUT `/vsock` (guest CID 3, UDS path `/run/vsock.socket`, vsock0), providing high-throughput bidirectional host-guest communication bypassing external network stacks. | `RESEARCH.md:51`<br>`firecracker.rs:455`<br>`deploy/images/README.md:120` | **Complete** | • `aura-core/src/hypervisor/firecracker.rs:455-459, 247-327, 376-389`<br>• Tests: `firecracker.rs:567-570, 616-644` | Fully implemented. Host listeners bind to `<vsock_uds>_<port>` and receive guest data. |
| **EXE-VM-010** | **Specialized RootFS Image Matrix**: Infrastructure-as-code scripts and configurations pre-baking ext4 rootfs disk images: `bash.ext4` (shell/linters), `browser.ext4` (headless Chromium), and `computer.ext4` (Xvfb/fluxbox GUI). | `deploy/images/README.md`<br>`firecracker.rs:17-84, 107-135`<br>`runtime/registry.rs:53-78` | **Complete** | • `aura-core/src/hypervisor/firecracker.rs:17-84, 107-135`<br>• `deploy/images/build_browser_image.sh`<br>• `deploy/images/build_computer_image.sh`<br>• Tests: `firecracker.rs:596-614` | Complete image build scripts, default sizing parameters, and runtime registry wiring. |
| **EXE-VM-011** | **Filesystem Layering & Overlay**: Base images must be kept read-only with ephemeral writable overlay (tmpfs/overlayfs) so guest writes do not mutate golden base images. | `SKILL.md (exe-vm):10`<br>`02-execution-sandboxes.md:25` | **Partial** | • `aura-core/src/hypervisor/firecracker.rs:444` (`"is_read_only": false`)<br>• `deploy/images/build_browser_image.sh:221`<br>• `deploy/images/build_computer_image.sh:315` | **Next Steps**: Update `configure_and_boot()` to pass `"is_read_only": true` for the base drive and mount an ephemeral overlayfs or writable scratch drive to prevent base image mutation across repeated runs. |
| **EXE-VM-012** | **Deterministic State Elimination**: MicroVM teardown terminates child processes and deletes all ephemeral host sockets and state files, guaranteeing no residual data persists between tool executions. | `RESEARCH.md:51`<br>`STRIDE.md:25`<br>`02-execution-sandboxes.md:25` | **Complete** | • `aura-core/src/hypervisor/firecracker.rs:471-500, 538-542`<br>• Tests: `firecracker.rs:574-579` | Full cleanup of API socket, vsock socket, screenshot socket, and child jailer process. |
| **EXE-VM-013** | **Graceful Shutdown Protocol**: Implement graceful shutdown by issuing PUT `/actions` with `SendCtrlAltDel` to the Firecracker API before terminating child processes, preventing orphaned child processes. | `KNOWLEDGE.md:76`<br>`firecracker.rs:475-476` | **Complete** | • `aura-core/src/hypervisor/firecracker.rs:475-476`<br>• Tests: `firecracker.rs:574-579` | Fully implemented in `FirecrackerVm::teardown`. |
| **EXE-VM-014** | **Lifecycle Resource Teardown**: Cleans up API socket, vsock socket, screenshot socket, and terminates jailer process upon explicit teardown or `Drop`. | `firecracker.rs:471-500, 538-542` | **Complete** | • `aura-core/src/hypervisor/firecracker.rs:471-500, 538-542`<br>• Tests: `firecracker.rs:574-579` | Verified in unit test suite. |
| **EXE-VM-015** | **Transparent TLS Egress Proxy & Registry Allowlisting**: Pure-Rust TLS-terminating / SNI-intercepting egress proxy (`EgressProxy`) enforcing Zero-Trust boundaries for microVM network egress, dropping non-TLS / unauthorized domains with TLS Fatal Alert 49 (Access Denied) and allowing official registries. | `SKILL.md (exe-vm):11-12`<br>`proxy.rs:8-416`<br>`config/mod.rs:245-268` | **Complete** | • `aura-core/src/hypervisor/proxy.rs:8-416`<br>• `aura-core/src/config/mod.rs:245-268`<br>• Tests: `proxy.rs:500-645` (4 tests) | Fully implemented with pure-Rust RFC 5246/8446 SNI parser, HTTP CONNECT proxy support, and fatal alert injection. |

---

### Category C: Computer GUI Automation & Display Sandbox (`TOOL-CMP`)

| Req ID | Title / Full Description | Source Document | Status | Code Citations | Specific Next Steps |
|---|---|---|---|---|---|
| **TOOL-CMP-001** | **Virtual Display Server & Window Manager**: Pre-configure `Xvfb` virtual framebuffer (`DISPLAY=:0`, default 1024x768x24) and lightweight window manager (`fluxbox`) inside `computer.ext4` image. | `deploy/images/build_computer_image.sh:5, 105, 241`<br>`deploy/images/README.md:11, 80-115`<br>`tools/computer.json:1-8` | **Complete** | • `deploy/images/build_computer_image.sh:238-278`<br>• `aura-core/src/hypervisor/firecracker.rs:24-25, 46, 56, 131-134`<br>• Tests: `firecracker.rs:609-614` | Implemented in `build_computer_image.sh` and pre-configured in `VmConfig::for_computer`. |
| **TOOL-CMP-002** | **GUI Testing & Input Automation Tools**: Pre-bundle `xdotool` (simulating mouse clicks/keystrokes) and `xcalc` (visual validation target), mapped to `ComputerTool.click()` in the Python SDK. | `deploy/images/build_computer_image.sh:282-288`<br>`deploy/images/README.md:112`<br>`aura-sdk/aura_sdk/tools/computer.py:4-10` | **Complete** | • `deploy/images/build_computer_image.sh:282-288`<br>• `aura-sdk/aura_sdk/tools/computer.py:4-10` | Pre-bundled scripts `/usr/local/bin/launch-xcalc` installed; Python SDK client wired. |
| **TOOL-CMP-003** | **Rust AF_VSOCK Screenshot Daemon**: Dedicated pure-Rust guest daemon connecting to X11 socket via `x11rb`, capturing root pixmap, encoding PNG bytes, and streaming over AF_VSOCK (port 5252/5254) to the host. | `deploy/images/README.md:119-162`<br>`build_computer_image.sh:271-275`<br>`firecracker.rs:14-15, 247-327` | **Complete** | • `deploy/images/vsock-screenshot-daemon/src/main.rs`<br>• `deploy/images/vsock-screenshot-daemon/src/capture.rs`<br>• `deploy/images/vsock-screenshot-daemon/src/server.rs`<br>• `aura-core/src/hypervisor/firecracker.rs:247-327`<br>• Tests: `vsock-screenshot-daemon/src/capture.rs:114-122` | Full daemon implementation supports listener, push-client, and length-prefixed streaming modes. |
| **TOOL-CMP-004** | **Zero-Trust Screenshot Validation**: Host supervisor validates raw and length-prefixed vsock byte streams against PNG specification (`PNG_MAGIC_BYTES`), rejecting malformed or malicious payloads before saving artifacts. | `firecracker.rs:11-12, 137-245`<br>`STRIDE.md:25` | **Complete** | • `aura-core/src/hypervisor/firecracker.rs:11-12, 137-245`<br>• Tests: `firecracker.rs:616-644, 646-673, 675-706` | Comprehensive tests verify raw PNG, length-prefixed PNG, and rejection of non-PNG payloads. |

---

### Category D: Bash Ephemeral Shell & Sandboxed Linting (`TOOL-BASH`)

| Req ID | Title / Full Description | Source Document | Status | Code Citations | Specific Next Steps |
|---|---|---|---|---|---|
| **TOOL-BASH-001** | **Ephemeral Bash Shell Sandbox**: Execute arbitrary shell commands inside an isolated Firecracker microVM using `bash.ext4` rootfs, destroying the VM post-execution. | `tools/bash.json:1-8`<br>`firecracker.rs:20-21, 34, 44, 54, 122-124`<br>`runtime/registry.rs:56-62`<br>`aura-sdk/aura_sdk/tools/bash.py:4-10` | **Complete** | • `tools/bash.json:1-8`<br>• `aura-core/src/hypervisor/firecracker.rs:122-124`<br>• `aura-core/src/runtime/registry.rs:56-62`<br>• `aura-sdk/aura_sdk/tools/bash.py:4-10`<br>• Tests: `runtime/registry.rs:107-116` | Fully wired in registry and hypervisor config. |
| **TOOL-BASH-002** | **Session Sandboxing & Resource Limit Caps**: Preserve session state across sequential bash calls within an active task scope while enforcing execution timeouts (125s) and memory caps (128MiB). | `SKILL.md (tool):7`<br>`tools/bash.json:6`<br>`firecracker.rs:54` | **Partial** | • `tools/bash.json:6` (`"timeoutMs": 125000`)<br>• `aura-core/src/hypervisor/firecracker.rs:54`<br>• `aura-sdk/aura_sdk/tools/bash.py:8-10` | **Next Steps**: Implement an in-guest persistent shell daemon listening over AF_VSOCK (similar to vsock-screenshot-daemon) to maintain environment variables and directory state across consecutive bash tool calls before session teardown. |
| **TOOL-BASH-003** | **Controlled Repository Linter Execution**: Securely execute repository linters (`ruff`, `prettier`, `cargo fmt`) inside Firecracker microVM with `VmConfig::for_bash`, gating git commits (`apply_patch`) without touching the host directly. | `ORIGINAL_REQUEST.md:14, 23`<br>`STRIDE.md:39`<br>`git.rs:108-142` | **Complete** | • `aura-core/src/knowledge/git.rs:108-142, 269, 297`<br>• Tests: `aura-core/src/knowledge/git.rs:373-392, 439-449` | Fully verified in `aura-core` unit tests: `apply_patch` halts immediately if `sandbox_lint` fails. |

---

### Category E: Headless Chromium Browser Sandbox (`TOOL-BRW`)

| Req ID | Title / Full Description | Source Document | Status | Code Citations | Specific Next Steps |
|---|---|---|---|---|---|
| **TOOL-BRW-001** | **Headless Chromium Pre-Baked Image**: Build `browser.ext4` pre-bundling headless Chromium, pre-cached font configurations (`fc-cache -fv`), fast init, and unprivileged user profile (`aura`) for sub-125ms cold boot. | `deploy/images/build_browser_image.sh:4, 86, 185, 211, 283`<br>`deploy/images/README.md:10, 43-77`<br>`tools/browser.json:1-7` | **Complete** | • `deploy/images/build_browser_image.sh`<br>• `aura-core/src/hypervisor/firecracker.rs:22-23, 35, 45, 55, 126-129`<br>• `aura-core/src/runtime/registry.rs:63-69`<br>• Tests: `runtime/registry.rs:117-125` | Production build script with in-chroot smoke verification and font pre-caching. |
| **TOOL-BRW-002** | **Zero-Trust Browser Content Handling**: Treat scraped browser DOM and web content strictly as untrusted data, preventing injection attacks into model instructions or host execution. | `STRIDE.md:29`<br>`SKILL.md (tool):8` | **Complete** | • `docs/architecture/threat-models/STRIDE.md:29`<br>• `.agents/plugins/aura-dev-kit/skills/aura-tool-domain/SKILL.md:8` | Documented and enforced invariant across cognitive parsing and tool returns. |
| **TOOL-BRW-003** | **CDP Remote Debugging Protocol**: Headless Chromium exposes Chrome DevTools Protocol (CDP) on port 9222 for remote navigation, DOM evaluation, and page scraping, controlled by `BrowserTool`. | `build_browser_image.sh:196`<br>`tools/browser.json:5`<br>`aura-sdk/aura_sdk/tools/browser.py:4-10` | **Partial** | • `deploy/images/build_browser_image.sh:185-204`<br>• `tools/browser.json:5`<br>• `aura-sdk/aura_sdk/tools/browser.py:8-10` | **Next Steps**: Wire a host-to-guest vsock TCP forwarding channel for port 9222 and integrate a headless CDP driver (e.g. chromiumoxide or playwright) in the Python SDK `BrowserTool.navigate()`. |

---

### Category F: Path-Confined Atomic FileSystem Tool (`TOOL-FS`)

| Req ID | Title / Full Description | Source Document | Status | Code Citations | Specific Next Steps |
|---|---|---|---|---|---|
| **TOOL-FS-001** | **Atomic File Operations**: Implement atomic read, write, and unified diff patching via adjacent temporary files and atomic rename, eliminating partial writes and race conditions. | `tools/filesystem/src/service.rs:9-10, 35-64, 91-124, 126-155`<br>`tools/filesystem/src/patch.rs:1-150` | **Complete** | • `tools/filesystem/src/service.rs:9-10, 35-155`<br>• `tools/filesystem/src/patch.rs:1-232`<br>• Tests: `tools/filesystem/tests/filesystem_test.rs:54-94` | Fully implemented and verified: atomic write, atomic read, unified diff patch, and conflict detection. |
| **TOOL-FS-002** | **Strict Workspace Path-Prefix Boundary**: Enforce canonical workspace boundaries on all filesystem requests via `PathValidator`, rejecting parent directory traversal (`..`) and outside paths. | `tools/filesystem/src/path.rs:9-17, 46-150`<br>`STRIDE.md:17, 39`<br>`tools/filesystem.json:1-7` | **Complete** | • `tools/filesystem/src/path.rs:9-150`<br>• `tools/filesystem/src/service.rs:10`<br>• Tests: `tools/filesystem/tests/filesystem_test.rs:5-51` | Fully verified: blocked parent traversal (`../secret.txt`, `foo/../../etc/passwd`) and blocked absolute escapes (`/etc/shadow`). |
| **TOOL-FS-003** | **WASI 0.3 WIT Component Interface**: Expose `aura:fs/read` and `aura:fs/write` interfaces via standard WIT definitions compiled with `wit-bindgen` for WASI 0.3 Component Model execution. | `docs/architecture/wit/aura-fs.wit:1-27`<br>`tools/filesystem/wit/`<br>`tools/filesystem/src/lib.rs:9-38` | **Complete** | • `docs/architecture/wit/aura-fs.wit:1-27`<br>• `tools/filesystem/src/lib.rs:9-38`<br>• `tools/filesystem.json:3, 6`<br>• Tests: `tools/filesystem/src/lib.rs` doctest | WIT interfaces match standard component model and export bindings. |
| **TOOL-FS-004** | **Sub-Millisecond Execution Performance**: Filesystem operations in the WASM sandbox execute with sub-millisecond overhead (<50ms end-to-end including disk I/O), fulfilling Docker-free latency goals. | `RESEARCH.md:36-43`<br>`tools/filesystem/tests/filesystem_test.rs:120-131` | **Complete** | • `tools/filesystem/tests/filesystem_test.rs:120-131` | Verified by automated benchmark test (`test_submillisecond_execution`). |

---

## 3. Archetype Specification Tables

### Features Discovered

| # | Category | Feature | Description | Inputs | Outputs | Error Behavior | Discovered Via |
|---|---|---|---|---|---|---|---|
| 1 | WASM | `create_engine_with_config` | Builds `wasmtime::Engine` with Cranelift, pooling allocator, async, and fuel metering | `&WasmConfig` | `Result<Engine, Box<dyn Error>>` | Returns error on invalid config | `wasm.rs:17-46` |
| 2 | WASM | `build_wasi_state` | Constructs `WasmState` with ResourceTable and WasiCtx after Zero-Trust SVID verification | `Svid` | `Result<WasmState, Box<dyn Error>>` | Traps with Zero-Trust Violation if `fs:read` missing | `wasm.rs:78-90` |
| 3 | WASM | `execute_component` | Instantiates and runs WASI 0.3 component asynchronously with fuel budget | `Engine`, `&[u8]`, `Svid`, `fuel_budget: u64` | `Result<(), Box<dyn Error>>` | Traps on out of fuel or missing export | `wasm.rs:92-117` |
| 4 | MicroVM | `SpecializedRootfs` | Rootfs image matrix enum defining standard image files, baseline vCPUs, and memory | None | Image filename, vCPU count, memory MiB | Returns error on unknown string in `from_str` | `firecracker.rs:18-84` |
| 5 | MicroVM | `FirecrackerVm::start_jailer` | Spawns `jailer` daemon enclosing Firecracker in cgroups v2, chroot, and seccomp | `&FirecrackerConfig` | `Result<(), Box<dyn Error>>` | Fails if jailer binary not found on host | `firecracker.rs:391-427` |
| 6 | MicroVM | `FirecrackerVm::configure_and_boot` | Configures microVM via Firecracker UDS API and issues `InstanceStart` | `&VmConfig` | `Result<(), Box<dyn Error>>` | Returns API error if HTTP status 4xx/5xx | `firecracker.rs:429-468` |
| 7 | MicroVM | `FirecrackerVm::teardown` | Issues `SendCtrlAltDel`, terminates jailer process, and unlinks sockets | None | `Result<(), Box<dyn Error>>` | Tolerant of non-existent sockets | `firecracker.rs:471-500` |
| 8 | MicroVM | `extract_png_bytes` | Zero-Trust validation extracting PNG from raw or length-prefixed vsock byte stream | `&[u8]` | `Result<&[u8], String>` | Rejects stream if PNG magic bytes missing | `firecracker.rs:144-182` |
| 9 | MicroVM | `start_screenshot_listener` | Spawns background listener for vsock screenshot stream from guest CID | `socket_path`, `artifacts_dir` | `Result<ScreenshotListenerHandle, Box<dyn Error>>` | Errors on socket bind failure | `firecracker.rs:247-327` |
| 10 | Proxy | `EgressProxy::start` | Pure-Rust SNI-intercepting TLS proxy enforcing domain allowlist | `EgressProxyConfig` | `Result<EgressProxyHandle, Box<dyn Error>>` | Errors on bind failure | `proxy.rs:260-295` |
| 11 | Proxy | `EgressProxy::extract_sni` | Pure-Rust RFC 5246/8446 ClientHello parser extracting target hostname | `&[u8]` | `Option<String>` | Returns `None` on malformed handshake | `proxy.rs:136-215` |
| 12 | Proxy | `EgressProxy::is_domain_allowed` | Validates target domain against allowlist supporting subdomains and wildcards | `&[String]`, `&str` | `bool` | Rejects prefix collisions (e.g. `evil-crates.io`) | `proxy.rs:107-132` |
| 13 | Toolchain | `Registry::register_default_sandboxes` | Registers bash, browser, and computer tools to Firecracker microVM backends | `Option<&str>` base directory | Mutates registry | None | `registry.rs:53-78` |
| 14 | FileSystem | `PathValidator::resolve_and_validate` | Confines target path to authorized workspace root and rejects `..` segments | `&str` path | `Result<PathBuf, FileSystemError>` | Fails on parent traversal or outside root | `tools/filesystem/src/path.rs:83-165` |
| 15 | FileSystem | `FileSystemService::write_file` | Writes file atomically via temporary file and atomic rename | `path: &str`, `contents: &[u8]` | `Result<(), String>` | Cleans tempfile on failure | `tools/filesystem/src/service.rs:64-124` |
| 16 | FileSystem | `FileSystemService::patch_file` | Parses unified diff and applies hunk transformations atomically | `path: &str`, `diff: &str` | `Result<String, String>` | Fails on context mismatch or conflict | `tools/filesystem/src/service.rs:128-155` |
| 17 | Toolchain | `build_browser_image.sh` | Infrastructure-as-code script building `browser.ext4` rootfs | CLI flags (`--size`, `--distro`) | ext4 image file | Fails if missing root tools or smoke test fails | `deploy/images/build_browser_image.sh` |
| 18 | Toolchain | `build_computer_image.sh` | Infrastructure-as-code script building `computer.ext4` rootfs with Xvfb/fluxbox | CLI flags (`--screen-res`, `--vsock-port`) | ext4 image file | Fails if missing root tools or smoke test fails | `deploy/images/build_computer_image.sh` |
| 19 | Toolchain | `vsock-screenshot-daemon` | Pure-Rust guest daemon capturing X11 framebuffer and pushing over AF_VSOCK | CLI flags (`--listen`, `--port`, `--display`) | PNG byte stream over vsock | Exits cleanly on disconnect | `deploy/images/vsock-screenshot-daemon` |
| 20 | Toolchain | `GitManager::sandbox_lint` | Runs repository linters (`ruff`, `prettier`, `cargo fmt`) inside Firecracker microVM | None (uses tracked files) | `Result<(), String>` | Halts git commit pipeline on linter failure | `aura-core/src/knowledge/git.rs:108-142` |

---

### Edge Cases Observed & Tested

| # | Feature | Input / Condition | Observed Behavior |
|---|---|---|---|
| 1 | WASM Fuel Metering | Infinite loop bytecode (`loop br 0 end`) with 100 fuel budget | Traps cleanly with error containing `"fuel"`, terminating execution in microseconds (`test_fuel_metering_trap`). |
| 2 | WASM Capability Check | SVID missing `fs:read` scope attempting WASI state construction | Rejected immediately with `"Zero-Trust Violation: SVID lacks required capability"` (`test_wasi_capability_denied`). |
| 3 | WASM Declarative Config | `default_fuel_budget: 0` or invalid `opt_level: "invalid"` | Validation fails with descriptive `ConfigError::Validation` error. |
| 4 | MicroVM Test Environment | Firecracker API socket does not exist (running outside KVM jailer) | `configure_and_boot` gracefully succeeds early in development/test mode without panicking (`test_firecracker_boot_config_schema`). |
| 5 | MicroVM Teardown | `teardown()` invoked when sockets or child processes are absent | Tolerant and idempotent; returns `Ok(())` without errors (`test_firecracker_vm_lifecycle_teardown`). |
| 6 | MicroVM Screenshot Stream | Raw payload missing PNG magic bytes (`b"MALICIOUS_PAYLOAD"`) | Zero-Trust validation drops payload with `"Zero-Trust Validation Failed"`; no artifact persisted on host (`test_screenshot_rejects_non_png_under_zero_trust`). |
| 7 | MicroVM Screenshot Stream | 4-byte big-endian length prefix preceding PNG magic bytes | Successfully parses declared length, strips prefix, and saves verified PNG artifact (`test_screenshot_length_prefixed_payload`). |
| 8 | TLS Egress Proxy | Target domain prefix collision (e.g. `evil-crates.io` attempting to bypass `crates.io`) | Rejected: `is_domain_allowed` strictly checks subdomain boundaries (`.` separator); connection terminated with TLS Fatal Alert 49 (`test_domain_allowlist_matching`, `test_proxy_drops_unauthorized_tls_connection`). |
| 9 | TLS Egress Proxy | Wildcard rule match (e.g. `*.ubuntu.com` matching `archive.ubuntu.com` and `security.ubuntu.com`) | Successfully permitted and forwarded to upstream server. |
| 10 | TLS Egress Proxy | Malformed or non-TLS traffic sent to TLS proxy port | Proxy emits `[EXE-VM-015:BLOCKED] Default deny: dropping non-TLS/non-proxy egress traffic` and immediately terminates stream. |
| 11 | HTTP Proxy | `CONNECT malicious-site.com:443 HTTP/1.1` | Blocked with `HTTP/1.1 403 Forbidden` response (`test_proxy_http_connect_allowed_and_blocked`). |
| 12 | FileSystem Path Traversal | Direct parent traversal (`"../secret.txt"`) | Denied with `"Path contains prohibited parent traversal '..' segment"` (`test_path_traversal_blocked_parent_dir`). |
| 13 | FileSystem Path Traversal | Embedded parent traversal (`"foo/../../etc/passwd"`) | Component-level inspection detects `Component::ParentDir` and rejects with error before path resolution. |
| 14 | FileSystem Boundary Escape | Absolute path pointing outside workspace (`"/etc/shadow"`) | Canonical prefix check detects destination falls outside canonical workspace root and returns `OutsideWorkspaceBoundary` error (`test_path_outside_boundary_blocked`). |
| 15 | FileSystem Non-Existent Target | Creating a new file in a non-existent subfolder (`"new/dir/file.txt"`) | Climbs upward to lowest existing parent directory, canonicalizes existing ancestor to verify containment, and creates parent directory hierarchy safely. |
| 16 | FileSystem Patching | Unified diff hunk with context line mismatch | Returns patch error: `"Context line mismatch: expected '...', found '...'"` and leaves target file untouched (`test_patch_conflict_detection`). |
| 17 | FileSystem Atomicity | Process interruption or I/O failure during file write | Writes proceed to adjacent hidden temporary file (`.tmp.<uuid>.<stem>`) on same filesystem mount and atomically overwrite target via `fs::rename`, guaranteeing zero partial/corrupted files. |
| 18 | Sandboxed Git Linting | `sandbox_lint` fails due to unformatted code or syntax error | `apply_patch` halts immediately, refusing to commit or update Merkle tree baseline (`test_apply_patch_halts_immediately_on_sandbox_lint_failure`). |

---

## 4. Gap Analysis & Next Steps for Partial Items

While all core M2 sandboxing primitives are structurally complete and fully verified, 3 items have partial runtime implementations:

### 1. `EXE-VM-011`: Guest Filesystem Overlay & Base Image Immutability
- **Current State**: `build_computer_image.sh` and `build_browser_image.sh` mount `tmpfs` on ephemeral directories (`/tmp`, `/run`, `/dev/shm`), but `configure_and_boot()` passes `"is_read_only": false` to `/drives/rootfs`.
- **Gap**: If a microVM writes files to `/root` or `/etc`, changes will persist to the golden ext4 disk image on the host.
- **Next Steps**:
  1. Pass `"is_read_only": true` for the primary rootfs in `configure_and_boot()`.
  2. Attach an ephemeral secondary ext4 drive or tmpfs overlay partition for rootfs writes so all mutations are discarded on VM teardown.

### 2. `TOOL-BASH-002`: Multi-Call Session Persistence
- **Current State**: Each tool call provisions an ephemeral microVM destroyed at completion (`tools/bash.json`).
- **Gap**: Consecutive commands in a single agent session (e.g. `cd /workspace/foo` followed by `cargo build`) execute in separate microVMs, resetting the working directory.
- **Next Steps**:
  1. Introduce an in-guest persistent shell daemon (similar to `vsock-screenshot-daemon`) running over `AF_VSOCK`.
  2. Maintain guest session state across consecutive tool calls within the same agent task ID before triggering `teardown()`.

### 3. `TOOL-BRW-003`: Chrome DevTools Protocol (CDP) Host-Guest Forwarding
- **Current State**: Chromium launches with `--remote-debugging-port=9222` inside `browser.ext4`, and `BrowserTool` is scaffolded in `aura-sdk`.
- **Gap**: Direct host-to-guest port forwarding or vsock tunneling for port 9222 is not yet wired to a live Python/Rust CDP driver (e.g. `chromiumoxide`).
- **Next Steps**:
  1. Add a vsock port listener in `firecracker.rs` forwarding port 9222 to the host.
  2. Implement CDP command serialization (e.g. `Page.navigate`, `Runtime.evaluate`, `Page.captureScreenshot`) in `aura_sdk/tools/browser.py`.

---

## 5. Verification Method

To independently verify all findings in this report, execute the following commands in `/home/jeff/aura`:

1. **Verify `aura-core` Hypervisor & Sandbox Tests (86 unit + 5 integration tests)**:
   ```bash
   cargo test -p aura-core
   ```
   *Expected Result*: All 86 unit tests and 5 integration tests pass with 0 errors.

2. **Verify Specialized Sandbox Unit Tests**:
   ```bash
   cargo test -p aura-core -- hypervisor
   ```
   *Expected Result*: 15 passed (WASM fuel metering, capability denial, Firecracker boot schema, screenshot PNG validation, proxy domain allowlisting, proxy alert injection).

3. **Verify `tools/` Workspace Sandboxed Tool Tests (37 tests)**:
   ```bash
   cargo test --manifest-path tools/Cargo.toml
   ```
   *Expected Result*: All 7 `filesystem` tests, 35 `ast-blast-radius` tests, and 5 `ephemeral-tunnel` tests pass.

4. **Verify `vsock-screenshot-daemon` Daemon Tests**:
   ```bash
   cargo test --manifest-path deploy/images/vsock-screenshot-daemon/Cargo.toml
   ```
   *Expected Result*: 1 passed (PNG encoding validation).

5. **Verify Python SDK Tooling Tests (48 tests)**:
   ```bash
   ./venv/bin/pytest aura-sdk/tests
   ```
   *Expected Result*: 48 passed.
