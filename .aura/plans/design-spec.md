# Aura CLI Architectural Design Specification: Phase 3

## 1. Overview
This document specifies Phase 3 of the `aura-cli` architecture, establishing the Two-Layer Zero-Trust Security model. It integrates dynamic SPIFFE workload identities and mutual TLS (mTLS) for transport-layer security (Layer 1), coupled with a Cedar-driven progressive authorization engine (Layer 2) that dynamically halts execution for human intervention.

## 2. Phase Breakdown and Module Boundaries

### Phase 3.1: Workload Identity (Layer 1)
- **Objective:** Establish dynamic X.509 cryptographic identities.
- **Modules (`src/ipc/zenoh.rs`):**
  - **SPIRE Agent Connection:** The `aura-cli` process connects to the local SPIRE agent endpoint via `WorkloadApiClient::connect_env()` during startup to fetch and maintain its SPIFFE Verifiable Identity Document (SVID).
  - **Identity Scope:** Target identity resolves to `spiffe://aura.local/workload/aura-cli`.

### Phase 3.2: mTLS Transport
- **Objective:** Encapsulate the Zenoh SHM topology within authenticated transport.
- **Modules (`src/ipc/zenoh.rs`):**
  - **Transport Wrapping:** Utilize `spiffe-rustls-tokio` to secure the Zenoh IPC connection.
  - **Zero-Trust Invariant:** Connections that lack a valid, unexpired SVID signed by the Aura root cluster authority are immediately dropped.

### Phase 3.3: Progressive Authorization (Layer 2)
- **Objective:** Evaluate autonomous tool invocations against immutable declarative policy schemas.
- **Modules (`src/auth/cedar.rs` and `src/ui/components/artifact_review.rs`):**
  - **Policy Engine:** Embed the Cedar policy engine (≥ 3.x) to evaluate incoming `aura/tools/{tool_name}/exec` requests.
  - **Authorization Tiers:**
    1. `strict`: Zero trust; all mutations require terminal confirmation.
    2. `request-review`: Default mode. Halts execution and presents the visual Artifact Review Pane.
    3. `proceed-in-sandbox`: Forwards request securely to Firecracker or WASI 0.3 environments.
    4. `always-proceed`: Automated execution strictly for CI/CD environments.
  - **Interactive Review:** The `AppModel` transitions into a review state when a forbid condition triggers under `request-review`, prompting the user via the `ratatui` interface to manually approve unified code diffs.

## 3. Substrate Evaluation
- **Execution Substrate:** `aura-cli` binary. Execution of accepted requests cascades to either lightweight WASI 0.3 WebAssembly runtimes or ephemeral Firecracker MicroVMs.
- **Model Routing Tier:** Gemini 3.1 Pro (Tier 3 architectural design allocation).

## 4. Invariants Verified
- **Docker-Free Bifurcated Sandboxing:** Authorization rules enforce that accepted tasks never route to traditional OCI containers.
- **SVID Attestation Requirement:** No unauthenticated process on the local loopback interface can spoof `aura-cli` Zenoh messages.

## 5. Verification Gates
1. **Gate 2: Attestation Dropping Test:** Simulate an untrusted local script pushing a message to the Zenoh topology without a SPIFFE SVID. The engine must reject the connection and log the `spiffe-rustls-tokio` drop natively.
2. **Cedar Policy Evaluation Assertion:** Assert that under the `request-review` default tier, a system-level tool execution (e.g., modifying `Cargo.toml`) returns a `Forbid` decision that triggers the Artifact Review Pane callback.

## 6. Rollback Strategy
- The implementation resides on the `feature/cli-security` branch.
- Should the local SPIRE agent fail to initialize consistently within the WSL2 network namespace, we will temporarily downgrade the Zenoh transport to loopback TCP to unblock development while maintaining Cedar's Layer 2 authorization engine.
