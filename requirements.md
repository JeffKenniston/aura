# Aura Project Master Requirements Specification (Phases M0–M3)

**Document Status:** Master Canonical Specification  
**Scope:** Phase M0 (Workspace Initialization & Core Primitives), Phase M1 (Kernel MVP / Transport & Identity Federation), Phase M2 (Execution Substrate / Docker-Free Sandboxing), and Phase M3 (Cognitive Processing & Observability)  
**Total Enumerated Requirements:** 121  

---

## Table of Contents
1. [Phase M0: Workspace Initialization & Core Primitives](#phase-m0-workspace-initialization--core-primitives)
2. [Phase M1: Kernel MVP / Transport & Identity Federation](#phase-m1-kernel-mvp--transport--identity-federation)
   - [HOST Domain: Execution Host & Microkernel Lifecycle](#host-domain-execution-host--microkernel-lifecycle)
   - [BUS Domain: Zenoh Event Bus & Routing Primitives](#bus-domain-zenoh-event-bus--routing-primitives)
   - [NET Domain: HTTP/3 QUIC Transport & MCP Layer](#net-domain-http3-quic-transport--mcp-layer)
   - [IAM Domain: SPIFFE/SPIRE Identity & Cedar Policy Enforcer](#iam-domain-spiffespire-identity--cedar-policy-enforcer)
   - [AGT Domain: Cognitive Scripting SDK](#agt-domain-cognitive-scripting-sdk)
   - [NFR-SEC Domain: Non-Functional Security Invariants](#nfr-sec-domain-non-functional-security-invariants)
3. [Phase M2: Execution Substrate / Docker-Free Sandboxing](#phase-m2-execution-substrate--docker-free-sandboxing)
   - [EXE-WASM Domain: WebAssembly WASI 0.3 Execution Substrate](#exe-wasm-domain-webassembly-wasi-03-execution-substrate)
   - [EXE-VM Domain: Firecracker MicroVM Hypervisor & Jailer Isolation](#exe-vm-domain-firecracker-microvm-hypervisor--jailer-isolation)
   - [TOOL-CMP Domain: Computer GUI Automation & Display Sandbox](#tool-cmp-domain-computer-gui-automation--display-sandbox)
   - [TOOL-BASH Domain: Bash Ephemeral Shell & Sandboxed Linting](#tool-bash-domain-bash-ephemeral-shell--sandboxed-linting)
   - [TOOL-BRW Domain: Headless Chromium Browser Sandbox](#tool-brw-domain-headless-chromium-browser-sandbox)
   - [TOOL-FS Domain: Path-Confined Atomic FileSystem Tool](#tool-fs-domain-path-confined-atomic-filesystem-tool)
4. [Phase M3: Cognitive Processing & Observability](#phase-m3-cognitive-processing--observability)
   - [COG Domain: Gemini Interactions API & Cognitive Processing](#cog-domain-gemini-interactions-api--cognitive-processing)
   - [RTE Domain: Model Router & Token Economics](#rte-domain-model-router--token-economics)
   - [GAP Domain: Context Saturation & Architectural Mitigations](#gap-domain-context-saturation--architectural-mitigations)

---

## Phase M0: Workspace Initialization & Core Primitives

| Requirement ID | Name | Full Description | Authoritative Source |
|---|---|---|---|
| **M0-SPIKE-01** | Cargo Workspace & Microkernel Layout | Establish an idiomatic multi-crate Rust workspace (`aura-core`, `aura-cli`) utilizing `resolver = "2"`, edition 2021, and centralized dependency management across shared libraries. | `ROADMAP.md` § Phase 0.1; `KNOWLEDGE.md:11-15`; `01-architecture-invariants.md:10-14` |
| **M0-SPIKE-02** | Isolated Python SDK Substrate | Establish an isolated `/sdk/python` (`aura-sdk`) runtime using standard `pyproject.toml` build system (Hatchling backend, Python >=3.11), Pydantic schemas, and strict process boundary separation from the Rust host. | `ROADMAP.md` § Phase 0.2; `KNOWLEDGE.md:16-18`; `01-architecture-invariants.md:15-18` |
| **M0-SPIKE-03** | Asynchronous Tokio Runtime Pool | Configure the Rust microkernel host with a multi-threaded Tokio runtime pool (`runtime::init_tokio_pool`) to manage concurrent I/O streams and asynchronous tasks without thread-blocking syscalls. | `ROADMAP.md` § Phase 0.3; `KNOWLEDGE.md:19-22`; `skills/aura-host-domain/SKILL.md:8` |
| **M0-SPIKE-04** | Zenoh Event Bus Integration | Integrate the Zenoh zero-overhead network protocol as an asynchronous, event-driven publish/subscribe and query data bus connecting the Rust host and Python agent runtimes with Zero Foreign Function Interfaces (Zero-FFI). | `ROADMAP.md` § Phase 0.4; `KNOWLEDGE.md:23-27`; `01-architecture-invariants.md:19-24` |

---

## Phase M1: Kernel MVP / Transport & Identity Federation

### HOST Domain: Execution Host & Microkernel Lifecycle

| Requirement ID | Name | Full Description | Authoritative Source |
|---|---|---|---|
| **HOST-001** | Microkernel Language | The execution host must be implemented in Rust to guarantee zero garbage collection pauses, deterministic latency, memory safety, and thread safety across multi-tenant workloads. | `aura-host-domain/SKILL.md:7`; `RESEARCH.md:11-13`; `01-architecture-invariants.md:11-14` |
| **HOST-002** | Asynchronous Concurrency Runtime | The host must run on a multi-threaded Tokio asynchronous runtime pool managing non-blocking concurrency without blocking worker threads on I/O or syscalls. | `aura-host-domain/SKILL.md:8`; `01-architecture-invariants.md:13`; `ADR-001:15-16` |
| **HOST-003** | In-Process Embedded WASM Engine | The host must embed the `wasmtime` runtime engine directly in-process with Cranelift JIT compilation, pooling instance allocation, and instruction fuel-metering enabled. | `aura-host-domain/SKILL.md:9`; `ROADMAP.md:19`; `KNOWLEDGE.md:37-41`; `ADR-001:10-12` |
| **HOST-004** | High-Concurrency Stream Multiplexing Scale | The host transport layer must multiplex 5,000+ concurrent HTTP/3 QUIC streams without thread starvation, worker depletion, or application-layer head-of-line blocking. | `aura-host-domain/SKILL.md:10`; `01-architecture-invariants.md:13`; `RESEARCH.md:12` |
| **HOST-005** | Cross-Platform OS Portability | The execution host must support native execution on Linux (x86_64 and aarch64) and Windows via WSL2 (supporting nested virtualization for `/dev/kvm`). | `aura-host-domain/SKILL.md:11`; `ROADMAP.md:23`; `RESEARCH.md:9, 49` |
| **HOST-006** | Zero Foreign Function Interface (Zero-FFI) | Direct C-FFI, PyO3, ctypes, or shared-memory pointers between the Rust host and Python cognitive agents are strictly prohibited; all communication proceeds strictly over Zenoh. | `aura-host-domain/SKILL.md:12`; `01-architecture-invariants.md:19-24` |
| **HOST-007** | Child Process Supervision & Lifecycle | The microkernel host must supervise external child processes (SPIRE agent, Jailer/Firecracker microVMs, cognitive agents), poll process health, and execute clean shutdown/restart procedures. | `aura-host-domain/SKILL.md:13`; `aura-core/src/lib.rs:39`; `aura-core/src/supervisor/mod.rs:64` |
| **HOST-008** | Declarative Microkernel Configuration | The host must load, validate, and enforce declarative configuration schemas (`AuraConfig`) from TOML and YAML configuration files, validating transport, bus, hypervisor, and supervisor parameters. | `aura-core/src/lib.rs:16`; `aura-core/src/config/mod.rs:57-114` |
| **HOST-009** | Administrative Drain & Graceful Shutdown | The host must support administrative `drain` commands and trap OS signals (SIGTERM/SIGINT), pausing new admissions while draining in-flight streams and tasks up to a configured drain timeout before shutdown. | `aura-host-domain/SKILL.md:14`; `aura-core/src/lib.rs:39`; `aura-core/src/supervisor/mod.rs:173-264` |
| **HOST-010** | Memory Safety & Zero GC Overhead | The host microkernel must eliminate runtime garbage collection overhead and memory corruption risks by leveraging strict Rust compile-time lifetimes, ownership semantics, and RAII guards. | `01-architecture-invariants.md:11-14`; `RESEARCH.md:12` |
| **HOST-011** | Hypervisor Substrate Backend Registry | The host must maintain an abstracted runtime registry (`Registry`) mapping tool invocation identifiers to specialized execution substrates (WASM component or Firecracker microVM). | `aura-core/src/runtime/registry.rs:16-78`; `ADR-001:10-12` |
| **HOST-012** | In-Flight Task Accounting & Guard RAII | The supervisor must maintain atomic in-flight task counters and issue RAII `TaskGuard` handles to prevent premature shutdown while background workloads execute. | `aura-core/src/supervisor/mod.rs:51-61, 98-115` |

### BUS Domain: Zenoh Event Bus & Routing Primitives

| Requirement ID | Name | Full Description | Authoritative Source |
|---|---|---|---|
| **BUS-001** | Scoped Pub/Sub Messaging | The event bus must provide publish and subscribe primitives strictly scoped under tenant and workspace key expression prefixes (`publish` and `subscribe`). | `aura-bus-domain/SKILL.md:7`; `aura-core/src/bus/mod.rs:45, 92, 107` |
| **BUS-002** | SPIFFE SVID Topic Scoping Boundary | Zenoh key expressions must be prefixed with the sanitized SPIFFE SVID of the executing workload (`aura/workspace/<svid>`), enforcing multi-tenant isolation at the topic layer. | `aura-core/src/bus/mod.rs:61-69`; `01-architecture-invariants.md:23` |
| **BUS-003** | Read-Switch-Execute-Return Task Dispatch & Completion | The bus must implement an asynchronous task queue listener on `tasks/dispatch` that routes invocations to sandboxes and emits completion events on `tasks/completion`. | `aura-bus-domain/SKILL.md:8`; `RESEARCH.md:19`; `aura-core/src/bus/mod.rs:125-167` |
| **BUS-004** | Standard Message Envelope Schema | All bus messages must encapsulate correlation ID, sender SVID, UTC timestamp, schema version (`1.0`), and payload, encrypted with TLS 1.3 / mTLS in transit. | `aura-bus-domain/SKILL.md:9`; `STRIDE.md:7`; `aura-sdk/aura_sdk/bus/client.py:72-121` |
| **BUS-005** | Topology: Router and Leaf Nodes | The microkernel must act as the central Zenoh router; sandboxes and cognitive agents connect strictly as leaf nodes without peer-to-peer mesh confusion. | `aura-bus-domain/SKILL.md:7`; `RESEARCH.md:19`; `01-architecture-invariants.md:23` |
| **BUS-006** | Key-Expression ACLs & Security Boundaries | The bus must enforce access control lists on key expressions matching SPIFFE identity, denying unauthorized cross-tenant topic access. | `aura-bus-domain/SKILL.md:10`; `STRIDE.md:6-7`; `aura-core/src/bus/mod.rs:61-69, 258-265` |
| **BUS-007** | At-Least-Once Delivery & Consumer Idempotency | Completion events are delivered at-least-once; consumers must maintain a bounded correlation ID cache to guarantee idempotent processing. | `aura-bus-domain/SKILL.md:11`; `aura-sdk/aura_sdk/bus/client.py:143-146, 174-186` |
| **BUS-008** | Token Bucket Event Bus Backpressure | The bus must enforce Token Bucket rate limiting on pub/sub streams, rejecting bursts that exceed capacity with `RateLimitExceeded`. | `ADR-001:17-18`; `aura-core/src/transport/rate_limiter.rs:13-64` |
| **BUS-009** | Per-Identity In-Memory Rate Limiting Guard | The host must enforce strict per-identity rate limits on tool-call events to prevent denial-of-service from runaway agents. | `STRIDE.md:12-13`; `aura-core/src/bus/mod.rs:245-253` |

### NET Domain: HTTP/3 QUIC Transport & MCP Layer

| Requirement ID | Name | Full Description | Authoritative Source |
|---|---|---|---|
| **NET-001** | Native HTTP/3 QUIC Transport Listener | The transport layer must bind an asynchronous UDP socket listener for native HTTP/3 QUIC transport using `quiche`. | `ROADMAP.md:13`; `aura-net-domain/SKILL.md:7`; `04-mcp-http3-transport.md:10-12` |
| **NET-002** | 0-RTT Connection Resumption | Enable 0-RTT connection resumption via TLS 1.3 session tickets (`enable_early_data()`) to eliminate handshake round-trips for returning agents. | `04-mcp-http3-transport.md:17`; `RESEARCH.md:88`; `aura-core/src/transport/http3.rs:31-32` |
| **NET-003** | Active Connection Migration | Support active connection migration across IP/network interface shifts using QUIC Connection IDs without terminating active streams. | `04-mcp-http3-transport.md:17`; `RESEARCH.md:90-91`; `aura-core/src/transport/http3.rs:34-35` |
| **NET-004** | ALPN Protocol Negotiation | Configure ALPN to advertise `h3` (primary) and fallback `h2` during TLS negotiation. | `RESEARCH.md:145`; `aura-core/src/transport/http3.rs:18-19` |
| **NET-005** | Silent TCP/HTTP-2 Fallback via Alt-Svc | Run a TCP listener advertising `Alt-Svc: h3=":4433"; ma=86400` headers to enable seamless 1-RTT degradation when UDP port 443 is blocked. | `ROADMAP.md:15`; `04-mcp-http3-transport.md:19-22`; `RESEARCH.md:141-146` |
| **NET-006** | HOL Blocking Elimination & Stream Isolation | Process QUIC streams independently via sans-I/O packet loops, eliminating TCP head-of-line blocking across distinct tool calls. | `aura-net-domain/SKILL.md:9, 10`; `04-mcp-http3-transport.md:14-17`; `STRIDE.md:5` |
| **NET-007** | Stateless July 2026 Model Context Protocol Server Interface | Provide a stateless Model Context Protocol (MCP) server conforming to the July 2026 specification using `serverUrl` endpoints. | `aura-net-domain/SKILL.md:8`; `04-mcp-http3-transport.md:10-13`; `RESEARCH.md:94-96` |

### IAM Domain: SPIFFE/SPIRE Identity & Cedar Policy Enforcer

| Requirement ID | Name | Full Description | Authoritative Source |
|---|---|---|---|
| **IAM-001** | Dynamic Workload Attestation via SPIRE Workload API | Dynamically fetch X.509 SVIDs from the local SPIRE agent over UNIX domain socket (`fetch_svid`), with graceful fallback for offline development. | `ROADMAP.md:16`; `aura-iam-domain/SKILL.md:7`; `03-zero-trust-identity.md:10-13` |
| **IAM-002** | Canonical SPIFFE SVID URI Schema | Parse and validate canonical SPIFFE URIs matching `spiffe://<trust-domain>/<tenant>/<kind>/<name>/<instance>`. | `aura-iam-domain/SKILL.md:8`; `03-zero-trust-identity.md:12`; `aura-core/src/identity/mod.rs:169-181` |
| **IAM-003** | Zero-Trust Default-Deny Authorization | Enforce default-deny authorization: workloads with empty or unverified scopes cannot execute any capability. | `aura-iam-domain/SKILL.md:9`; `03-zero-trust-identity.md:14-18`; `aura-core/src/identity/mod.rs:182-195` |
| **IAM-004** | Explicit Capability Scopes & Non-Escalation | Capabilities are explicitly declared; child SVID scopes must be strict subsets of parent scopes, and runtime scope broadening is strictly forbidden. | `aura-iam-domain/SKILL.md:9, 10`; `STRIDE.md:10-11`; `aura-core/src/identity/mod.rs:19-74` |
| **IAM-005** | Explicit ScopeDenied & Cedar Policy Evaluation | Evaluate Cedar policies against principal, action, and resource, returning structured `403 ScopeDenied` on authorization failure. | `STRIDE.md:11`; `aura-core/src/identity/mod.rs:80-98, 415-484` |
| **IAM-006** | Filesystem Path-Prefix Boundary Matching & Anti-Traversal | Audit filesystem operations ensuring target paths begin with authorized prefixes and reject relative `..` path traversals. | `aura-iam-domain/SKILL.md:11`; `03-zero-trust-identity.md:16`; `STRIDE.md:17` |
| **IAM-007** | mTLS Peer Identity & Trust Domain Enforcement | Enforce mutual TLS peer validation during handshakes, verifying peer certificates against the expected trust domain (`aura.local`). | `STRIDE.md:5`; `aura-core/src/identity/mod.rs:633-663` |
| **IAM-008** | Enterprise Trust Domain Federation Manager | Maintain cross-domain trust bundles (`FederationManager`) to support enterprise federated identities across multiple trust domains. | `ADR-002:10-11`; `deploy/spire/README.md:3`; `aura-core/src/identity/mod.rs:666-694` |

### AGT Domain: Cognitive Scripting SDK

| Requirement ID | Name | Full Description | Authoritative Source |
|---|---|---|---|
| **AGT-001** | Scripting Substrate Language | High-level agent behaviors must be scripted in Python (>=3.11) with Pydantic typed data validation. | `aura-agt-domain/SKILL.md:7`; `01-architecture-invariants.md:15-18` |
| **AGT-002** | Process Isolation & Zero-FFI Communication | Cognitive agents must run in dedicated OS processes communicating with the microkernel strictly over Zenoh. | `aura-agt-domain/SKILL.md:8`; `01-architecture-invariants.md:19-24` |
| **AGT-003** | Asynchronous Agent Primitives | Provide core asynchronous agent primitives: `call_tool()`, `dispatch_background()`, `await_event()`, and `emit()`. | `aura-agt-domain/SKILL.md:9`; `aura-sdk/aura_sdk/bus/client.py:245-350` |
| **AGT-004** | Non-Blocking Zero-CPU Suspension | Agents awaiting events must yield CPU execution completely using `asyncio.Future` until messages arrive over Zenoh. | `aura-agt-domain/SKILL.md:9`; `RESEARCH.md:19`; `aura-sdk/aura_sdk/bus/client.py:263` |
| **AGT-005** | Workload SVID Attestation & Attachment | Agents must obtain an SVID at startup, parse constituent fields, and attach the identity to every outbound bus envelope. | `aura-agt-domain/SKILL.md:10`; `aura-sdk/aura_sdk/bus/client.py:34-58, 73-121` |
| **AGT-006** | Agent Dependency Isolation | Pin and isolate agent Python dependencies within dedicated virtual environments. | `aura-agt-domain/SKILL.md:11`; `aura-sdk/pyproject.toml:19-48` |
| **AGT-009** | Dynamic Workspace Path Resolution | Dynamically compute authorized workspace filesystem paths based on tenant and agent name extracted from SVID (`/workspace/<tenant>/<name>`). | `aura-sdk/aura_sdk/bus/client.py:60-69, 150-153` |
| **AGT-010** | State-Checkpointing Decorator | Provide `@state_checkpoint` decorator to serialize local function state and context to Zenoh before long-running task suspension. | `aura-agt-domain/SKILL.md:12`; `aura-sdk/aura_sdk/bus/client.py:482-521` |
| **AGT-011** | In-Memory Bus Mock Mode for Offline Tests | Provide an in-memory bus mock mode (`AURA_MOCK_BUS=1`) allowing offline unit testing without a live Zenoh daemon. | `aura-sdk/aura_sdk/bus/client.py:136-138, 241-244` |

### NFR-SEC Domain: Non-Functional Security Invariants

| Requirement ID | Name | Full Description | Authoritative Source |
|---|---|---|---|
| **NFR-SEC-001** | Zero-Trust Identity Verification | Every active workload, tool, and microVM must present a valid SVID issued via SPIRE under a default-deny capability model. | `03-zero-trust-identity.md:10-18`; `STRIDE.md:4-5`; `deploy/spire/README.md:3` |
| **NFR-SEC-002** | Granular Scope Enforcement & Anti-Escalation | Scopes explicitly declare allowed tools; runtime escalation is blocked and immediately returns `403 ScopeDenied`. | `STRIDE.md:10-11`; `aura-iam-domain/SKILL.md:9-10`; `aura-core/src/identity/mod.rs:80-98` |
| **NFR-SEC-003** | Cryptographic Transit Encryption & Envelope Integrity | All transport (HTTP/3, TCP fallback) and Zenoh traffic must be encrypted with TLS 1.3 / mTLS; messages carry signed correlation IDs and timestamps. | `STRIDE.md:7`; `aura-bus-domain/SKILL.md:10`; `aura-net-domain/SKILL.md:10` |
| **NFR-SEC-004** | Sandbox & Path Boundary Containment | The host must enforce path-prefix boundary matching, prevent relative traversal (`..`), and isolate execution substrates. | `STRIDE.md:16-17`; `03-zero-trust-identity.md:16`; `aura-core/src/identity/mod.rs:364-413` |

---

## Phase M2: Execution Substrate / Docker-Free Sandboxing

### EXE-WASM Domain: WebAssembly WASI 0.3 Execution Substrate

| Requirement ID | Name | Full Description | Authoritative Source |
|---|---|---|---|
| **EXE-WASM-001** | Docker-Free WebAssembly Runtime Engine | Embed `wasmtime` within `aura-core`, providing a Docker-free isolated sandbox configured for WASI 0.3 execution. | `ROADMAP.md:19`; `RESEARCH.md:30-34`; `ADR-001:10-11`; `SKILL.md (exe-wasm):7` |
| **EXE-WASM-002** | Cranelift JIT Compiler Optimization | Configure Cranelift compiler optimization levels (`OptLevel::Speed`, `SpeedAndSize`, `None`) via `WasmConfig` for near-native execution speed. | `ROADMAP.md:19`; `RESEARCH.md:32`; `SKILL.md (exe-wasm):9` |
| **EXE-WASM-003** | Tokio Asynchronous Integration | Enable asynchronous execution (`async_support(true)`), natively bridging WASI 0.3 completion streams and futures to the Tokio event loop. | `ROADMAP.md:20`; `RESEARCH.md:30-34`; `SKILL.md (exe-wasm):8` |
| **EXE-WASM-004** | WASI 0.3 Component Model Support | Configure native WASM Component Model support (`wasm_component_model(true)`), enforcing canonical ABI compliance for completion-based asynchronous I/O with `WasiView` state. | `ROADMAP.md:20`; `RESEARCH.md:36-44`; `02-execution-sandboxes.md:16` |
| **EXE-WASM-005** | Instruction Fuel Metering | Enforce Cranelift instruction fuel consumption (`consume_fuel(true)`) and dynamic budget injection, deterministically trapping runaway execution loops. | `ROADMAP.md:19`; `RESEARCH.md:32`; `STRIDE.md:19`; `02-execution-sandboxes.md:18` |
| **EXE-WASM-006** | High-Concurrency Pooling Instance Allocator | Configure `wasmtime::InstanceAllocationStrategy::Pooling` for sub-millisecond instance instantiation and memory slot reuse across concurrent workers. | `ROADMAP.md:19`; `RESEARCH.md:32, 41`; `SKILL.md (exe-wasm):9`; `02-execution-sandboxes.md:17` |
| **EXE-WASM-007** | Asynchronous Linker & Capability Attestation | Build asynchronous WASI component linker (`add_to_linker_async`) with mandatory Zero-Trust capability validation (`CapabilityEnforcer::check`) preventing ambient authority. | `SKILL.md (exe-wasm):11`; `STRIDE.md:17`; `02-execution-sandboxes.md:15` |
| **EXE-WASM-008** | Component Run Dispatch & Static Composition Invariant | Asynchronously instantiate components and invoke standard entrypoints (`wasi:cli/run@0.2.0#run`), enforcing deploy-time static composition without runtime hot-loading. | `RESEARCH.md:148-150`; `02-execution-sandboxes.md:19`; `SKILL.md (exe-wasm):12` |

### EXE-VM Domain: Firecracker MicroVM Hypervisor & Jailer Isolation

| Requirement ID | Name | Full Description | Authoritative Source |
|---|---|---|---|
| **EXE-VM-001** | Hardware-Isolated KVM Virtualization | Provision Firecracker microVMs over Linux Kernel Virtual Machine (`/dev/kvm`), booting isolated VirtIO guest kernels without Docker or OCI containers. | `ROADMAP.md:21`; `RESEARCH.md:47-49`; `ADR-001:10-12`; `02-execution-sandboxes.md:21-23` |
| **EXE-VM-002** | Nested Virtualization Host Validation | Validate `/dev/kvm` availability and permissions for nested virtualization inside host environments (WSL2 / Linux servers). | `ROADMAP.md:23`; `KNOWLEDGE.md:45`; `RESEARCH.md:49`; `deploy/images/README.md:37` |
| **EXE-VM-003** | Jailer Daemon Isolation | Spawn Firecracker wrapped by the `jailer` daemon, applying cgroups v2 limits, dedicated chroot directory tree, namespace isolation, UID/GID dropping, and strict seccomp syscall filtering. | `ROADMAP.md:22`; `RESEARCH.md:51`; `STRIDE.md:23`; `02-execution-sandboxes.md:24`; `SKILL.md (exe-vm):8` |
| **EXE-VM-004** | Unix Domain Socket API Client | Communicate with the Firecracker API over Unix Domain Socket (`root/run/firecracker.socket`), issuing raw HTTP/1.1 PUT requests (`/boot-source`, `/drives/rootfs`, `/machine-config`, `/vsock`, `/actions`). | `KNOWLEDGE.md:46`; `aura-core/src/hypervisor/firecracker.rs:429-468` |
| **EXE-VM-005** | Minimal VirtIO Sizing & Boot Overhead | Configure minimal machine parameters (<5MiB memory overhead, <125ms cold boot, baseline 1 vCPU, 128MiB RAM for lightweight workloads). | `ROADMAP.md:21`; `RESEARCH.md:49`; `STRIDE.md:23`; `02-execution-sandboxes.md:23` |
| **EXE-VM-006** | Session-Scoped Ephemeral Lifecycle | MicroVMs are ephemeral and provisioned per tool-call or task session, preventing cross-session data bleeding and guaranteeing pristine isolation. | `ROADMAP.md:21`; `STRIDE.md:25`; `02-execution-sandboxes.md:25`; `SKILL.md (exe-vm):9` |
| **EXE-VM-007** | RootFS Drive Attachment | Dynamically configure the root block device via PUT `/drives/rootfs`, binding host ext4 disk images to guest `/dev/vda`. | `aura-core/src/hypervisor/firecracker.rs:442`; `deploy/images/README.md:177` |
| **EXE-VM-008** | Cold Boot Action Dispatch | Boot microVMs using Firecracker `InstanceStart` action; aligns with ADR-001 cold boot decision without complex snapshot pooling overhead. | `ADR-001:13-14`; `aura-core/src/hypervisor/firecracker.rs:460` |
| **EXE-VM-009** | VirtIO AF_VSOCK Multiplexing | Configure virtio-vsock device via PUT `/vsock` (guest CID 3, UDS path `/run/vsock.socket`), providing high-throughput bidirectional host-guest communication bypassing external network stacks. | `RESEARCH.md:51`; `aura-core/src/hypervisor/firecracker.rs:455`; `deploy/images/README.md:120` |
| **EXE-VM-010** | Specialized RootFS Image Matrix | Pre-bake specialized ext4 rootfs disk images: `bash.ext4` (shell/linters), `browser.ext4` (headless Chromium), and `computer.ext4` (Xvfb/fluxbox GUI). | `deploy/images/README.md`; `aura-core/src/hypervisor/firecracker.rs:17-84` |
| **EXE-VM-011** | Filesystem Layering & Overlay | Base images must be kept read-only with ephemeral writable overlay (tmpfs/overlayfs) so guest writes do not mutate golden base images. | `SKILL.md (exe-vm):10`; `02-execution-sandboxes.md:25` |
| **EXE-VM-012** | Deterministic State Elimination | MicroVM teardown terminates child processes and deletes all ephemeral host sockets and state files, guaranteeing no residual data persists between executions. | `RESEARCH.md:51`; `STRIDE.md:25`; `02-execution-sandboxes.md:25` |
| **EXE-VM-013** | Graceful Shutdown Protocol | Implement graceful shutdown by issuing PUT `/actions` with `SendCtrlAltDel` to the Firecracker API before terminating child processes. | `KNOWLEDGE.md:76`; `aura-core/src/hypervisor/firecracker.rs:475-476` |
| **EXE-VM-014** | Lifecycle Resource Teardown | Clean up API socket, vsock socket, screenshot socket, and terminate jailer process upon explicit teardown or `Drop`. | `aura-core/src/hypervisor/firecracker.rs:471-500, 538-542` |
| **EXE-VM-015** | Transparent TLS Egress Proxy & Registry Allowlisting | Pure-Rust TLS-terminating / SNI-intercepting egress proxy (`EgressProxy`) enforcing Zero-Trust boundaries for microVM network egress, dropping unauthorized domains with TLS Fatal Alert 49 (Access Denied). | `SKILL.md (exe-vm):11-12`; `aura-core/src/hypervisor/proxy.rs:8-416`; `aura-core/src/config/mod.rs:245-268` |

### TOOL-CMP Domain: Computer GUI Automation & Display Sandbox

| Requirement ID | Name | Full Description | Authoritative Source |
|---|---|---|---|
| **TOOL-CMP-001** | Virtual Display Server & Window Manager | Pre-configure `Xvfb` virtual framebuffer (`DISPLAY=:0`, default 1024x768x24) and lightweight window manager (`fluxbox`) inside `computer.ext4` image. | `deploy/images/build_computer_image.sh:5, 105, 241`; `deploy/images/README.md:11`; `tools/computer.json:1-8` |
| **TOOL-CMP-002** | GUI Testing & Input Automation Tools | Pre-bundle `xdotool` (simulating mouse clicks/keystrokes) and `xcalc` (visual validation target), mapped to `ComputerTool.click()` in the Python SDK. | `deploy/images/build_computer_image.sh:282-288`; `deploy/images/README.md:112`; `aura-sdk/aura_sdk/tools/computer.py:4-10` |
| **TOOL-CMP-003** | Rust AF_VSOCK Screenshot Daemon | Dedicated pure-Rust guest daemon connecting to X11 socket via `x11rb`, capturing root pixmap, encoding PNG bytes, and streaming over AF_VSOCK (port 5252/5254) to the host. | `deploy/images/README.md:119-162`; `build_computer_image.sh:271-275`; `aura-core/src/hypervisor/firecracker.rs:14-15, 247-327` |
| **TOOL-CMP-004** | Zero-Trust Screenshot Validation | Host supervisor validates raw and length-prefixed vsock byte streams against PNG specification (`PNG_MAGIC_BYTES`), rejecting malformed or malicious payloads before saving artifacts. | `aura-core/src/hypervisor/firecracker.rs:11-12, 137-245`; `STRIDE.md:25` |

### TOOL-BASH Domain: Bash Ephemeral Shell & Sandboxed Linting

| Requirement ID | Name | Full Description | Authoritative Source |
|---|---|---|---|
| **TOOL-BASH-001** | Ephemeral Bash Shell Sandbox | Execute arbitrary shell commands inside an isolated Firecracker microVM using `bash.ext4` rootfs, destroying the VM post-execution. | `tools/bash.json:1-8`; `aura-core/src/hypervisor/firecracker.rs:122-124`; `aura-core/src/runtime/registry.rs:56-62` |
| **TOOL-BASH-002** | Session Sandboxing & Resource Limit Caps | Preserve session state across sequential bash calls within an active task scope while enforcing execution timeouts (125s) and memory caps (128MiB). | `SKILL.md (tool):7`; `tools/bash.json:6`; `aura-core/src/hypervisor/firecracker.rs:54` |
| **TOOL-BASH-003** | Controlled Repository Linter Execution | Securely execute repository linters (`ruff`, `prettier`, `cargo fmt`) inside Firecracker microVM with `VmConfig::for_bash`, gating git commits (`apply_patch`) without touching the host directly. | `ORIGINAL_REQUEST.md:14, 23`; `STRIDE.md:39`; `aura-core/src/knowledge/git.rs:108-142` |

### TOOL-BRW Domain: Headless Chromium Browser Sandbox

| Requirement ID | Name | Full Description | Authoritative Source |
|---|---|---|---|
| **TOOL-BRW-001** | Headless Chromium Pre-Baked Image | Build `browser.ext4` pre-bundling headless Chromium, pre-cached font configurations (`fc-cache -fv`), fast init, and unprivileged user profile (`aura`) for sub-125ms cold boot. | `deploy/images/build_browser_image.sh:4, 86, 185`; `deploy/images/README.md:10`; `tools/browser.json:1-7` |
| **TOOL-BRW-002** | Zero-Trust Browser Content Handling | Treat scraped browser DOM and web content strictly as untrusted data, preventing injection attacks into model instructions or host execution. | `STRIDE.md:29`; `SKILL.md (tool):8` |
| **TOOL-BRW-003** | CDP Remote Debugging Protocol | Headless Chromium exposes Chrome DevTools Protocol (CDP) on port 9222 for remote navigation, DOM evaluation, and page scraping, controlled by `BrowserTool`. | `deploy/images/build_browser_image.sh:196`; `tools/browser.json:5`; `aura-sdk/aura_sdk/tools/browser.py:4-10` |

### TOOL-FS Domain: Path-Confined Atomic FileSystem Tool

| Requirement ID | Name | Full Description | Authoritative Source |
|---|---|---|---|
| **TOOL-FS-001** | Atomic File Operations | Implement atomic read, write, and unified diff patching via adjacent temporary files and atomic rename, eliminating partial writes and race conditions. | `tools/filesystem/src/service.rs:9-10, 35-64, 91-124`; `tools/filesystem/src/patch.rs:1-150` |
| **TOOL-FS-002** | Strict Workspace Path-Prefix Boundary | Enforce canonical workspace boundaries on all filesystem requests via `PathValidator`, rejecting parent directory traversal (`..`) and outside paths. | `tools/filesystem/src/path.rs:9-17, 46-150`; `STRIDE.md:17, 39`; `tools/filesystem.json:1-7` |
| **TOOL-FS-003** | WASI 0.3 WIT Component Interface | Expose `aura:fs/read` and `aura:fs/write` interfaces via standard WIT definitions compiled with `wit-bindgen` for WASI 0.3 Component Model execution. | `docs/architecture/wit/aura-fs.wit:1-27`; `tools/filesystem/wit/`; `tools/filesystem/src/lib.rs:9-38` |
| **TOOL-FS-004** | Sub-Millisecond Execution Performance | Filesystem operations in the WASM sandbox execute with sub-millisecond overhead (<50ms end-to-end including disk I/O), fulfilling Docker-free latency goals. | `RESEARCH.md:36-43`; `tools/filesystem/tests/filesystem_test.rs:120-131` |

---

## Phase M3: Cognitive Processing & Observability

### COG Domain: Gemini Interactions API & Cognitive Processing

| Requirement ID | Name | Full Description | Authoritative Source |
|---|---|---|---|
| **COG-001** | Gemini Interactions API Exclusive Usage | The cognitive subsystem must strictly utilize Google's Gemini Interactions API (`/v1beta/interactions`) for all agentic reasoning and interaction turns, deprecating and prohibiting legacy `generateContent` stateless endpoints. | `ROADMAP.md` Phase 3 Item 1; `RESEARCH.md` Section 53-55; `aura-cog-domain/SKILL.md` Rule 1; `05-gemini-interactions-routing.md` Section 1 |
| **COG-002** | Server-Side State Management via previous_interaction_id | Follow-up interaction turns must pass `previous_interaction_id` instead of resending previous turn history, leveraging server-side conversation state to maximize cache hit rates, decrease TTFT, and minimize token spend. | `ROADMAP.md` Phase 3 Item 1; `RESEARCH.md` Section 56-58; `aura-cog-domain/SKILL.md` Rule 2; `05-gemini-interactions-routing.md` Section 1 |
| **COG-003** | Server-Sent Events (SSE) Streaming Ingestion | The client must consume streaming SSE events from the Interactions API and expose structured typed events (`cog.thought`, `cog.tool_call`, `cog.text_delta`, `cog.interaction_created`, `cog.interaction_completed`, `cog.error`) to agents. | `ROADMAP.md` Phase 3 Item 2; `RESEARCH.md` Section 59-60; `aura-cog-domain/SKILL.md` Rule 3; `05-gemini-interactions-routing.md` Section 1 |
| **COG-004** | Tool & Function Calling Sandbox Interception & Reinjection | Intercept `function_call` steps emitted by the model, execute the requested tool securely inside isolated WASM or Firecracker sandboxes, and reinject `function_result` payloads back into the same interaction session via `previous_interaction_id`. | `ROADMAP.md` Phase 3 Item 2; `RESEARCH.md` Section 59-60; `aura-cog-domain/SKILL.md` Rule 4; `05-gemini-interactions-routing.md` Section 1 |
| **COG-005** | Long-Horizon Background Tasks (background=true) | Long-horizon autonomous workflows must specify `background=true`, detaching the request from standard HTTP timeouts and tracking completion via Zenoh status topics or polling. | `ROADMAP.md` Phase 3 Item 5; `RESEARCH.md` Section 61-64; `aura-cog-domain/SKILL.md` Rule 5; `05-gemini-interactions-routing.md` Section 1 |
| **COG-006** | Native Retrieval-Augmented Generation via file_search_stores | Integrate native RAG using the `file_search_stores` abstraction in Gemini interaction payloads, allowing agents to pull multimodal context without requiring a host-managed external vector database. | `RESEARCH.md` Section 68-70; `aura-cog-domain/SKILL.md` Rule 6; `05-gemini-interactions-routing.md` Section 1 |
| **COG-007** | Token Economics, TTFT, and Cost Metrics | Record granular metrics per interaction turn, including TTFT (Time-to-First-Token), prompt tokens, candidate tokens, cached content tokens, total tokens, and USD cost calculated according to model tier pricing tables. | `RESEARCH.md` Section 57, 73-80; `07-guardrails-token-economics.md`; `aura-sdk/aura_sdk/cognition/gemini.py` |
| **COG-008** | Transient API Error Retry with Jittered Exponential Backoff | Automatically retry transient network and provider errors (HTTP 429 ResourceExhausted, 500, 502, 503, 504 Unavailable) using jittered exponential backoff up to a configurable maximum retry count. | `aura-sdk/aura_sdk/cognition/gemini.py`; `aura-sdk/tests/test_cognition_gemini.py` |
| **COG-009** | API Credential Leak Prevention & Sanitization | Prevent long-lived API keys, Bearer tokens, or cloud secrets from entering the model prompt context, agent memory, or telemetry logs. Pre-flight scanning must reject leaks with `SecurityCredentialLeakError`. | `RESEARCH.md` Section 134-137; `STRIDE.md` Boundary 5 (TB5); `aura-cog-domain/SKILL.md` Rule 7 |
| **COG-010** | Tool Call Argument Schema Validation | Validate model-generated tool call arguments against registered JSON Schema definitions before executing tools in sandboxes, raising `ToolArgumentValidationError` on missing required fields or type mismatches. | `aura-sdk/aura_sdk/cognition/gemini.py`; `aura-sdk/tests/test_cognition_gemini.py` |
| **COG-011** | File Search Store Deletion & Lifecycle Management | Explicitly call Gemini API deletion endpoints (`HTTP DELETE /v1beta/fileSearchStores/{name}`) upon task completion or TTL expiration, ensuring ephemeral RAG stores do not accumulate storage costs. | `aura-cog-domain/SKILL.md` Rule 6; `aura-sdk/aura_sdk/cognition/gemini.py` |
| **COG-012** | Tenant Billing Tag Injection | Inject tenant billing identifiers (`billing_tenant_id` and `x-tenant-id`) into request client metadata for all outbound interaction calls to enable downstream multi-tenant billing attribution. | `aura-sdk/aura_sdk/cognition/gemini.py`; `aura-sdk/tests/test_cognition_gemini.py` |
| **COG-013** | Host-Side Semantic Vector Prompt Cache | SQLite vector database with `sqlite-vec` extension and BLAKE3 hashing in the Rust microkernel (`aura-core`), queried over Zenoh by Python SDK (`SemanticCacheClient`) before calling Gemini; returns exact/semantic duplicates in <50ms with 0 token spend. | `KNOWLEDGE.md` Section 70; `aura-core/src/knowledge/cache.rs`; `aura-sdk/aura_sdk/cognition/cache.py` |
| **COG-014** | Shadow Tracker File Storage Calculations | The host-side `ShadowTracker` and SDK client must calculate cumulative Gemini API file storage costs by correlating uploaded object sizes and TTLs against configured provider pricing rates. | `ROADMAP.md` Phase 3 Item 4; `RESEARCH.md` Section 152-157; `aura-core/src/config/mod.rs`; `aura-core/src/transport/rate_limiter.rs` |

### RTE Domain: Model Router & Token Economics

| Requirement ID | Name | Full Description | Authoritative Source |
|---|---|---|---|
| **RTE-001** | Four Supported Model Tiers | The model router must define and support four distinct model tiers: Pro (`gemini-3.1-pro`), Flash (`gemini-3.8-flash`), Flash-Lite (`gemini-3.5-flash-lite`), and Deep Research (`deep-research-preview`). | `ROADMAP.md` Phase 3 Item 3; `RESEARCH.md` Section 71-80; `aura-rte-domain/SKILL.md` Rule 1; `05-gemini-interactions-routing.md` Section 2 |
| **RTE-002** | Default Orchestration Tier (Flash) | The router must select Flash (`gemini-3.8-flash`) as the default tier for standard software engineering workflows, feature additions, tool orchestrations, and testing. | `RESEARCH.md` Section 77; `aura-rte-domain/SKILL.md` Rule 1; `05-gemini-interactions-routing.md` Section 2 |
| **RTE-003** | Cognitive Demand, Latency, and Cost Heuristics | Deterministic rule-based decision engine evaluating task requirements: route to Pro for high cognitive demand (planning, architecture, deep multi-step reasoning, or prompts > 2000 chars); route to Flash-Lite for latency constraints (< 1000ms), cost constraints (< $0.001), or lightweight validation/parsing tasks. | `ROADMAP.md` Phase 3 Item 3; `RESEARCH.md` Section 73-80; `ADR-002` Decision 2 |
| **RTE-004** | Explicit Gating for Deep Research | Deep Research (`deep-research-preview`) shall ONLY be invoked through an explicit agent option (`agent_options={"deep_research": True}` or `agent_option="deep-research"`), never autonomously selected by general keyword matching in strict mode. | `RESEARCH.md` Section 79; `aura-rte-domain/SKILL.md` Rule 1; `05-gemini-interactions-routing.md` Section 2 |
| **RTE-005** | AST Blast Radius Cognitive Escalation | When the proposed code modification's reverse-dependency blast radius meets or exceeds the threshold (default: 3 reverse-dependencies), the router must automatically escalate from Flash to Pro tier (`gemini-3.1-pro`). | `RESEARCH.md` Section 130-132; `ADR-002` Decision 2; `aura-rte-domain/SKILL.md` Rule 2; `05-gemini-interactions-routing.md` Section 2 |
| **RTE-006** | Hot Configuration Reload | Support runtime hot reloading of tier model mappings and blast radius threshold parameters from configuration dicts or TOML strings without code changes or service restarts. | `aura-sdk/aura_sdk/routing/router.py`; `aura-sdk/tests/test_router.py` |
| **RTE-007** | Decision Audit Logging | Every model routing decision must be logged with its selected tier, model ID, specific decision rationale, tenant ID, timestamp, and metadata for auditability. | `RESEARCH.md` Section 67; `aura-sdk/aura_sdk/routing/router.py` |
| **RTE-008** | Per-Tenant Token and Cost Budgets | Enforce per-tenant soft limits (which trigger warnings) and hard limits (which raise `BudgetExceededError`, blocking all new interactions). | `RESEARCH.md` Section 134-137; `aura-rte-domain/SKILL.md` Rule 3; `aura-sdk/aura_sdk/routing/router.py` |
| **RTE-009** | 90% Cost Budget Webhook & Task Queue Suspension | When a tenant reaches 90% of their hard cost budget, trigger a synchronous HTTP billing webhook; if denied or timed out, block Pro tier interactions (`ProTierBlockedError`) in the SDK, and suspend the tenant's Zenoh queue via the host's `QueueSuspensionManager`, terminating the agent if approval fails. | `RESEARCH.md` Section 134-137; `aura-rte-domain/SKILL.md` Rule 3; `aura-core/src/config/mod.rs`; `aura-core/src/transport/rate_limiter.rs`; `aura-core/src/transport/webhook.rs` |

### GAP Domain: Context Saturation & Architectural Mitigations

| Requirement ID | Name | Full Description | Authoritative Source |
|---|---|---|---|
| **GAP-001** | UDP Port 443 Middlebox & DPI Blocking | Corporate middleboxes and deep packet inspection (DPI) filter or drop UDP/QUIC traffic on port 443, risking permanent transport stalls. | `RESEARCH.md` Section 141-144; `04-mcp-http3-transport.md` Section 3; `aura-net-domain/SKILL.md` |
| **GAP-002** | 1-RTT Silent TCP Fallback via Alt-Svc / ALPN | Detect UDP failure within 1 RTT and transparently degrade MCP transport from HTTP/3 to HTTP/2 over TCP using Alt-Svc headers without dropping agent requests. | `ROADMAP.md` Phase 1 Item 3; `RESEARCH.md` Section 144-146; `04-mcp-http3-transport.md` Section 3 |
| **GAP-003** | WASI 0.3 Component Model Dynamic Linking Limitation | Standard WASI 0.3 lacks standardized dynamic runtime hot-linking of newly generated WASM components without pre-composition or host restart. | `RESEARCH.md` Section 147-151; `ADR-001` |
| **GAP-004** | Heavyweight & Dynamic Tool Execution in Ephemeral MicroVMs | Tools requiring native shell access, GUI, or complex python packages cannot execute in WASM; must run in ephemeral Firecracker microVMs wrapped by jailer with vsock streaming. | `RESEARCH.md` Section 46-52; `ROADMAP.md` Phase 2 Items 3-4; `ADR-001` |
| **GAP-005** | Context Window Saturation & State Expansion in Long-Horizon Workflows | For continuous long-horizon workflows using `background=true`, stateful conversation history in `previous_interaction_id` accumulates until exceeding model context limits (1M-2M tokens), risking catastrophic token exhaustion and cache invalidation. | `RESEARCH.md` Section 152-157; `ADR-002` Decision 4; `05-gemini-interactions-routing.md` Section 3; `aura-sdk/aura_sdk/cognition/gemini.py`: Line 3 |
| **GAP-006** | 80% Saturation Detection, Task Pause, and Automated Flash-Lite Summarization | At 80% context window saturation (e.g. 800k / 1M tokens), automatically pause the active task, emit `context/pause` event over Zenoh, dispatch a summarization prompt to Flash-Lite (`gemini-3.5-flash-lite`), seed a new Interaction ID (`seed-<uuid>`), and emit `context/resume`. | `RESEARCH.md` Section 156-157; `ADR-002` Decision 4; `05-gemini-interactions-routing.md` Section 3; `aura-sdk/aura_sdk/cognition/context.py`; `aura-sdk/aura_sdk/routing/router.py` |
| **GAP-007** | Distilled Prompt Context Handoff | Prepend the dense conversation summary into the subsequent task prompt instruction (`format_resumed_prompt`) and resume execution under the newly seeded interaction session without resending the uncompressed historical chain. | `RESEARCH.md` Section 156-157; `ADR-002` Decision 4; `aura-sdk/aura_sdk/cognition/context.py` |
| **GAP-008** | Hard Limit Guard Against Recursive Infinite Summarization | Enforce a strict hard limit on consecutive summarization loops (configurable default 3, ceiling 5 per ADR-002) to prevent infinite loops where repeated summarization consumes tokens endlessly on circular tasks. Exceeding the limit raises `InfiniteSummarizationError` and aborts task execution. The counter is reset upon normal turn completion (`reset_consecutive_loops()`). | `ADR-002` Decision 4; `07-guardrails-token-economics.md` Section 1; `aura-sdk/aura_sdk/cognition/context.py` |
