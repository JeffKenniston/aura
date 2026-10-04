---
command: /plan
description: Mandates structured architectural design specification and risk assessment before code modifications.
version: 1.0
---

# /plan Workflow: Architectural Implementation Specification

When invoked, the agent must NOT modify any code. Instead, execute the following protocol:

1. **Architecture Discovery**:
   - Identify module ownership: Rust microkernel (`aura-core`), Python substrate (`aura-sdk`), CLI (`aura-cli`), or sandboxes (`tools/`).
   - Check invariants: Ensure zero FFI between Rust and Python (Zenoh bus only), Docker-free execution, and valid SPIFFE identity.

2. **AST Blast-Radius Pre-Check**:
   - Execute CTE query on `.agents/knowledge_graph.sqlite` to identify upstream callers.
   - If blast radius > 10 callers, escalate to Gemini 3.1 Pro and notify developer.

3. **Substrate Selection**:
   - Assign WASI 0.3 WebAssembly for lightweight tasks; assign Firecracker microVM for OS integration tasks.

4. **Deliverable**:
   - Write design specification to `.aura/plans/design-spec.md` with verification gates and rollback plan.
   - Await developer approval before proceeding.
