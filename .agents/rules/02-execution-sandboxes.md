---
title: Docker-Free Execution Sandboxing (WASM and Firecracker)
description: Defines the bifurcated sandboxing strategy using WASI 0.3 WebAssembly and ephemeral Firecracker microVMs.
tags: [sandboxing, wasm, wasi, firecracker, jailer, kvm]
version: 1.0
---

# Docker-Free Execution Sandboxing

## Invariant: Absolute Prohibition of Legacy Containers
- Docker, containerd, and standard OCI container runtimes are strictly forbidden from the runtime architecture.
- Workloads are partitioned into lightweight functional components and heavyweight OS integrations.

## 1. Lightweight Workloads: WebAssembly & WASI 0.3
- Target tasks: Pure computational utilities, AST transformations, credential filtering, and filesystem path-prefix operations.
- **Component Model**: Built against WASI 0.3, utilizing native asynchronous streams and futures in the ABI to eliminate readiness polling.
- **Engine**: Hosted in `wasmtime` with Cranelift JIT compilation and pooling allocators for sub-millisecond instantiation.
- **Fuel Metering**: Strict instruction fuel-metering is enforced to terminate runaway executions deterministically.
- **Dynamic Linking Restriction**: Static component composition post-deployment must be utilized until Component Model 1.0 formally standardizes dynamic host linking.

## 2. Heavyweight Workloads: Firecracker MicroVMs
- Target tasks: Shell commands (`Bash`), package installations, headless browser testing (`Browser` / Chromium), and GUI interactions (`Computer`).
- **Hypervisor**: Linux Kernel Virtual Machine (KVM) API booting distinct VirtIO-centric guest kernels in <125ms with <5MiB memory overhead.
- **Jailer Wrapper**: Every Firecracker instance must be enclosed within a `jailer` daemon enforcing cgroups v2 resource limits and strict seccomp system-call filter whitelists.
- **Ephemeral Lifecycle**: MicroVMs are ephemeral and provisioned per tool-call. They must be destroyed immediately upon completion to guarantee deterministic isolation.
