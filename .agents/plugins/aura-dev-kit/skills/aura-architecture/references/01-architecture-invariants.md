---
title: Architecture Invariants and Kernel Decoupling
description: Enforces the decoupled Rust host and Python scripting boundaries via Zenoh event bus with zero FFI.
tags: [architecture, rust, python, zenoh, microkernel]
version: 1.0
---

# Architecture Invariants & Kernel Decoupling

## Invariant 1: Microkernel Layer Separation
- **Execution Host (Rust - `aura-core`)**:
  - Must act as the hypervisor supervisor, networking engine, memory manager, and cryptographic workload attestation host.
  - Built on an asynchronous `tokio` runtime; leverages memory-safe idioms without garbage collection overhead.
  - Hosts the `wasmtime` runtime and controls Firecracker microVM lifecycles.
- **Cognitive Scripting Substrate (Python - `aura-sdk`)**:
  - Defines agent cognitive behaviors, heuristic routing logic, and interfaces with AI/ML ecosystems.
  - Must be treated as non-privileged with respect to hardware and hypervisor management.

## Invariant 2: Zero Foreign Function Interface (FFI)
- Under no circumstances shall direct C-FFI, PyO3, or ctypes foreign function interfaces be used for runtime communication between the Rust microkernel and Python agents.
- All inter-process communication must proceed across an asynchronous, event-driven publish/subscribe data bus powered by **Zenoh**.
- Rationale: Python dependency crashes or segmentation faults must never compromise the underlying Rust orchestration kernel.
- Python agents and execution sandboxes operate strictly as Zenoh leaf nodes subscribing to designated task queues.
