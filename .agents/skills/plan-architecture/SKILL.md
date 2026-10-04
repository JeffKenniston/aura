---
name: plan-architecture
description: Evaluates proposed system modifications against Aura's decoupled microkernel architecture and drafts formal implementation plans adhering to the /plan protocol.
version: 1.0
---

# Architecture Planning Skill (`/plan`)

## Overview
This skill guides the agent in systematically analyzing architectural modifications before executing file changes. It enforces Aura's microkernel invariants, selects appropriate execution substrates, and allocates cognitive model tiers.

## Workflow Instructions
1. **Deconstruct User Objective**:
   - Analyze whether the requested feature touches the **Execution Host (Rust)**, the **Cognitive Scripting Layer (Python)**, or shared **Zenoh Bus schemas**.
   - Verify that no direct Foreign Function Interfaces (FFI) are introduced.

2. **Substrate Evaluation**:
   - For lightweight/utility tasks, designate **WASI 0.3 WebAssembly** running in `wasmtime` with fuel metering.
   - For OS-level tasks (Bash, Chromium), designate ephemeral **Firecracker MicroVMs** with jailer seccomp/cgroup configurations.

3. **Cognitive Tier Allocation**:
   - Assign appropriate model tiers according to complexity:
     - Tier 3 / Architectural planning: Gemini 3.1 Pro.
     - Feature implementation & tooling: Gemini 3.8 Flash.
     - Validation & log inspection: Gemini 3.5 Flash-Lite.

4. **Deliverable Generation**:
   - Output a formal plan to `.aura/plans/design-spec.md` detailing:
     - Target components and module boundaries.
     - Invariants verified (SPIFFE SVID scoping, HTTP/3 transport, zero FFI).
     - Step-by-step verification gates.
     - Explicit rollback strategy.
