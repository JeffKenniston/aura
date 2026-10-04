# /plan: Zero-Trust SPIFFE/SPIRE Provisioning & Enforcement

## 1. Architecture Discovery
- **Module Ownership:** Rust microkernel (`aura-core`), specifically expanding `identity/mod.rs` to include a `CapabilityEnforcer` and injecting hooks into the `hypervisor/wasm.rs` boundary.
- **Invariants Checked:** 
  - *Zero-Trust Cryptographic Identity:* A valid SVID is no longer just fetched; it must be actively audited. No capability (e.g., FileSystem writes, Network bindings) is granted unless explicitly authorized.
  - *Docker-Free Sandboxing:* The enforcement must occur at the WASI 0.3 WebAssembly level and the Firecracker microVM setup level.

## 2. AST Blast-Radius Pre-Check
- **Status:** Evaluated against `.agents/knowledge_graph.sqlite`.
- **Blast Radius:** 0 upstream callers are structurally dependent on the nonexistent `CapabilityEnforcer`. Safe to implement.

## 3. Substrate Selection
- **Substrate:** Native Rust Execution Host (`aura-core`).
- **Implementation Strategy:**
  - Expand the `Svid` struct to expose explicit authorization scopes (e.g., parsing SPIRE selectors or internal permission arrays).
  - Implement a `CapabilityEnforcer` in Rust that takes an `Svid` and a requested `Operation` (like `FileSystemWrite` or `NetworkOpen`).
  - Wire this enforcer directly into `hypervisor/wasm.rs`. When building the `WasiCtxBuilder`, conditionally grant directory access or network access only if the enforcer approves.
  - Unprivileged WASM components attempting an unauthorized capability will be blocked at instantiation or execution.

## 4. Verification Gates
1. **Enforcer Scaffolding:** Create `aura-core/src/identity/enforcer.rs` to evaluate capabilities.
2. **SVID Scope Expansion:** Modify `fetch_svid()` to extract or map specific capabilities (mocking SPIFFE selectors where necessary for the test environment).
3. **WASI Capability Hooks:** Inject `CapabilityEnforcer::check()` into the WASI context builder in `wasm.rs`.
4. **Validation:** Write a unit test `test_wasi_capability_denied` proving that a WASM component without filesystem capabilities securely traps or fails instantiation when attempting unauthorized access.

## 5. Rollback Strategy
- Atomic Git commits scoping the WASI changes.
- If parsing raw X.509 extensions for SPIRE selectors proves unsupported by the current `spiffe` v0.16.1 crate without deep C-FFI parsing, we will rollback and implement a pure-Rust JWT-SVID validation path as the fallback to maintain the zero-trust invariants.
