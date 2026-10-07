# AGENTS.md — Foundational Repository Constitution for Aura

## 1. Core Architecture Invariants (Unbreakable)
Aura is a decoupled, local-first multi-agent orchestration OS. You MUST respect these invariants:
1. **Decoupled Boundaries (Zero FFI)**: Rust (`aura-core`) handles execution/resources; Python (`aura-sdk`) handles cognitive logic. Communication is STRICTLY via the Zenoh pub/sub bus. No direct FFI.
2. **Docker-Free Sandboxing**: No traditional containers. Use WASI 0.3 for lightweight tasks; Firecracker microVMs for heavy tasks.
3. **Zero-Trust Identity**: All execution contexts require a SPIFFE SVID.
4. **Context & Rules**: Detailed architecture rules, design invariants, and domain vocabulary are maintained in the `aura-architecture` skill. **ACTIVATE the `aura-architecture` skill** when working on the kernel, transport, identity, sandboxes, or executing structural refactors.

## 2. Operational Guardrails
1. **Three-Round Rule**: Halt execution and ask the developer if an issue isn't resolved in 3 iterations. No speculative permutations.
2. **Verification Gates**: 
   - You absolutely MUST fully implement everything with functional, production-ready code--ABSOLUTELY NO non-functional code, mocks, stubs, or TODOs!
   - Code formatting/linting is handled deterministically via `PostToolUse` lifecycle hooks (`cargo`, `ruff`). Do not attempt to manually format code.
   - Structural refactors must undergo AST Blast-Radius CTE analysis before applying.
