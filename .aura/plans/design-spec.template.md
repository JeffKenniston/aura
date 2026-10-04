# Architectural Design Specification Template

## 1. Feature Objective & Scope
- **Target Component**: [aura-core | aura-sdk | aura-cli | tools]
- **Cognitive Model Tier**: [Gemini 3.1 Pro | Gemini 3.8 Flash | Gemini 3.5 Flash-Lite]

## 2. Invariants Check
- [ ] Decoupled Rust/Python boundaries (Zenoh bus, Zero FFI)
- [ ] Docker-free sandboxing (WASI 0.3 / Firecracker MicroVM)
- [ ] SPIFFE/SPIRE SVID attestation verified
- [ ] HTTP/3 QUIC transport with 1-RTT Alt-Svc TCP fallback verified

## 3. AST Blast-Radius Assessment
- **Target Symbols**:
- **Downstream Callers Count**:
- **Risk Level**: [Low | Medium | High]

## 4. Implementation Steps & Verification Gates
1. Step 1:
2. Verification: Local binary check (`cargo check` / `ruff check`)
