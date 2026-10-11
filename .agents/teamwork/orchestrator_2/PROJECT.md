# Project: Aura M0-M3 Codebase Audit & Gap Analysis

## Architecture
- **M0 (Spikes / Workspace Initialization)**:
  - Cargo workspace initialization, Python SDK packaging with Poetry, Tokio runtime async event loop, Zenoh zero-overhead pub/sub data bus.
- **M1 (Kernel MVP / Transport & Identity Federation)**:
  - HOST Domain (HOST-001..012): Microkernel layer separation, tokio concurrency, embedded wasmtime, process supervision, graceful drain, declarative configuration.
  - BUS Domain (BUS-001..009): SVID topic scoping, task queue dispatch, rate limiting, idempotency tracking.
  - NET Domain (NET-001..007): MCP over HTTP/3 via QUIC, 0-RTT resumption, connection migration, ALPN negotiation, Alt-Svc fallback to HTTP/2.
  - IAM Domain (IAM-001..008): SPIFFE/SPIRE dynamic SVID workload attestation, Cedar authorization policies, ephemeral child SVID minting, mTLS.
  - AGT Domain (AGT-001..006, 009..011): Decoupled Python cognitive substrate, Zenoh bus client bindings, zero FFI.
  - NFR-SEC Domain (NFR-SEC-001..004): Zero-trust network boundaries, default-deny egress, data integrity.
- **M2 (Sandboxes / Docker-Free Sandboxing)**:
  - EXE-WASM Domain (EXE-WASM-001..008): WASI 0.3 component model, Cranelift JIT, fuel metering, pooling allocator.
  - EXE-VM Domain (EXE-VM-001..015): Firecracker microVMs, Jailer daemon, UDS API, AF_VSOCK multiplexing, TLS egress proxy.
  - Sandboxed Toolchain: TOOL-CMP-001..004 (Computer GUI & vsock screenshot daemon), TOOL-BASH-001..003 (Bash & linter isolation), TOOL-BRW-001..003 (Headless Chromium), TOOL-FS-001..004 (Path-confined atomic FileSystem).
- **M3 (Cognition / Cognitive Processing & Observability)**:
  - COG Domain (COG-001..014): Gemini Interactions API, previous_interaction_id turns, SSE stream ingestion, function call interception, background tasks, RAG file search stores, credential leak protection, semantic cache, shadow storage tracking.
  - RTE Domain (RTE-001..009): Multi-tiered routing (Pro, Flash, Flash-Lite, Deep Research), AST blast radius escalation, tenant budgets, 90% cost webhook & queue suspension.
  - GAP Domain (GAP-001..008): 80% context saturation detection, Flash-Lite summarization & seed ID restart, recursive loop guard.

## Feature Inventory Summary
- Total Requirements Surveyed: 121
  - M0 (Spikes): 4 requirements (M0-SPIKE-01..04) [3 Complete, 1 Partial]
  - M1 (Kernel MVP): 49 requirements (HOST 12, BUS 9, NET 7, IAM 8, AGT 9, NFR-SEC 4) [44 Complete, 5 Partial]
  - M2 (Sandboxes): 37 requirements (EXE-WASM 8, EXE-VM 15, TOOL-CMP 4, TOOL-BASH 3, TOOL-BRW 3, TOOL-FS 4) [33 Complete, 4 Partial]
  - M3 (Cognition): 31 requirements (COG 14, RTE 9, GAP 8) [30 Complete, 1 Partial]
  - Total: 110 Complete (90.9%), 11 Partial (9.1%), 0 Missing (0.0%).

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| Survey | Requirements Mining | Parallel survey of M0, M1, M2, and M3 | none | DONE |
| M-SYNTH | Report & Requirements Synthesis | Generate requirements.md and M0_M3_Audit_Report.md | Survey | DONE |
| M-VERIFY | Multi-Agent Verification Gate | Reviewers, Challengers, and Forensic Auditor verification | M-SYNTH | DONE (Gate PASSED) |
| M-DELIVER | Sentinel Completion Notification | Deliver final report to Sentinel | M-VERIFY | DONE |
