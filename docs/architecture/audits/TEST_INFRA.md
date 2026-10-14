# E2E Test Infra: Aura Phase M5 (Hardening)

## Test Philosophy
- Opaque-box, requirement-driven. Derived strictly from ORIGINAL_REQUEST.md without coupling to internal implementation details.
- Systematic 4-tier methodology: Category-Partition, Boundary Value Analysis, Pairwise Combinatorial Testing, and Real-World Workload Testing.
- Progressive testability: verification mechanisms must not depend on features more complex than what they verify.

## Feature Inventory
| # | Feature | Source (Requirement) | Tier 1 | Tier 2 | Tier 3 |
|---|---------|----------------------|:------:|:------:|:------:|
| 1 | Ephemeral Credential Redaction Formatting | ORIGINAL_REQUEST §R1 | 5 | 5 | ✓ |
| 2 | SPIFFE Scope Enforcement & Non-Escalation | ORIGINAL_REQUEST §R1 | 5 | 5 | ✓ |
| 3 | Ephemeral Tunnel Service Revocation Lifecycle | ORIGINAL_REQUEST §R1 | 5 | 5 | ✓ |
| 4 | Out-of-Band Sandbox Credential Injection | ORIGINAL_REQUEST §R1 | 5 | 5 | ✓ |
| 5 | Non-Ambient VCS PR Creation (GitManager::open_pr) | ORIGINAL_REQUEST §R1 | 5 | 5 | ✓ |
| 6 | Guest Daemon X11 Synthetic Actions Protocol | ORIGINAL_REQUEST §R2 | 5 | 5 | ✓ |
| 7 | Screen Capture & X11 Depth Handling | ORIGINAL_REQUEST §R2 | 5 | 5 | ✓ |
| 8 | Host-Side PNG Scanline Visual Redaction | ORIGINAL_REQUEST §R2 | 5 | 5 | ✓ |
| 9 | Python SDK ComputerTool Actions | ORIGINAL_REQUEST §R2 | 5 | 5 | ✓ |
| 10 | Vsock Stream Buffer Memory Limits & Truncation | ORIGINAL_REQUEST §R3 | 5 | 5 | ✓ |
| 11 | MicroVM Egress Transparent TLS Enforcement | ORIGINAL_REQUEST §R3 | 5 | 5 | ✓ |
| 12 | Encrypted Database at Rest (AES-256-GCM) | ORIGINAL_REQUEST §R3 | 5 | 5 | ✓ |
| 13 | Task Ledger Encrypted Persistence | ORIGINAL_REQUEST §R3 | 5 | 5 | ✓ |
| 14 | SPIRE Trust Domain Federation & OIDC Verification | ORIGINAL_REQUEST §R4 | 5 | 5 | ✓ |
| 15 | Transport UDP 1-RTT Failure Detection & TCP Fallback | ORIGINAL_REQUEST §R4 | 5 | 5 | ✓ |

## Test Architecture
- Test runner: `cargo test --test m5_e2e_opaque_test` & `pytest aura-sdk/tests/test_computer_tool.py`
- Test cases validate exit codes, JSON-RPC schema compliance, cryptographic ciphertexts at rest, and transport fallback.
- Directory layout:
  - `aura-core/tests/m5_e2e_opaque_test.rs`
  - `tools/ephemeral-tunnel/tests/`
  - `deploy/images/vsock-screenshot-daemon/tests/`
  - `aura-sdk/tests/test_computer_tool.py`

## Real-World Application Scenarios (Tier 4)
| # | Scenario | Features Exercised | Complexity |
|---|----------|--------------------|------------|
| 1 | End-to-End Secure PR Workflow: Mint ephemeral token with GitHub scope -> Inject out-of-band to sandbox -> Run lint -> Commit & Open PR -> Verify token auto-revoked across all registries | F1, F2, F3, F4, F5 | High |
| 2 | Visual GUI Automation with Sensitive Redaction: ComputerTool sends mouse moves/clicks to Firecracker vsock daemon -> captures full screen -> applies redaction masks over sensitive credentials -> verifies visual blackout & PNG valid header | F6, F7, F8, F9 | High |
| 3 | Rogue Guest Stream & Egress Violation Defense: Rogue microVM attempts to stream 64MB over vsock -> host truncates payload without crash; MicroVM attempts egress to unauthorized host -> receives TLS Alert 49 | F10, F11 | High |
| 4 | Encrypted Ledger Audit & Zero-Knowledge Verification: Task lifecycle events appended to ledger -> Database saved to disk -> Raw byte scan verifies ciphertext (no plaintext table names or task payloads) -> Reopened with SPIFFE key | F12, F13 | High |
| 5 | Enterprise Federation & Transport Self-Healing: Enterprise OIDC token verified against SPIRE federation mapping -> Tool call issued over HTTP/3 -> Simulated UDP blackhole triggers fallback to HTTP/2 TCP within 1 RTT with zero dropped calls | F14, F15 | High |

## Coverage Thresholds
- Tier 1: >=5 per feature (75 tests across 15 features)
- Tier 2: >=5 per feature (75 tests across boundary/corner conditions)
- Tier 3: pairwise cross-feature interaction tests (>=15 tests)
- Tier 4: >=5 realistic end-to-end application scenarios
- **Total Minimum Test Count: ~170 tests**
