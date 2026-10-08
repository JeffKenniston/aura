# Aura Threat Model (STRIDE)

## Boundary 1: Client ↔ Host (TB1)
*   **Spoofing:** A malicious client attempts to spoof another agent's identity.
    *   **Control (NET-006):** Enforce strict mTLS using SPIFFE-bound X.509 SVIDs or OAuth tied to SPIFFE.
*   **Tampering:** Modifying Zenoh bus messages in transit.
    *   **Control (BUS-004, NFR-SEC-003):** All traffic is encrypted with TLS 1.3 / mTLS. Messages carry a signed correlation ID and timestamp.

## Boundary 2: Agent ↔ Host (TB2)
*   **Elevation of Privilege:** An agent attempts to expand its assigned scopes to access forbidden tools.
    *   **Control (IAM-004, IAM-005):** Scoped SVIDs explicitly declare allowed tools. Scopes can never be broadened at runtime. Attempting to call an unauthorized tool returns `ScopeDenied`.
*   **Denial of Service:** An agent floods the host with tool-call events.
    *   **Control (BUS-009):** The host enforces a strict in-memory rate limit per agent identity, rejecting excess traffic with `RateLimitExceeded`.

## Boundary 3: WASM Guest ↔ Host (TB3)
*   **Escape / Information Disclosure:** A WASM component attempts to read files outside its workspace or escape the sandbox.
    *   **Control (EXE-WASM-007):** WASI 0.3 capabilities are host-granted only. Path-prefix matching (TOOL-FS-002) is strictly enforced on all filesystem operations.
*   **Denial of Service:** A WASM component goes into an infinite loop.
    *   **Control (EXE-WASM-005):** Strict Cranelift instruction fuel-metering traps and terminates the instance when the budget is exhausted.

## Boundary 4: MicroVM ↔ Host (TB4)
*   **Escape:** A `Bash` or `Browser` microVM guest attempts to break into the Rust host.
    *   **Control (EXE-VM-005):** KVM isolation is wrapped by Firecracker's `jailer`, enforcing cgroups v2 resource limits, chroot, namespaces, and a strict seccomp syscall filter.
*   **Cross-Session Contamination:** Data from one tool call bleeds into another.
    *   **Control (EXE-VM-006, EXE-VM-012):** MicroVMs are ephemeral. If recycling is eventually implemented, the guest is subjected to a full reset. High-risk tools always receive a fresh VM.

## Boundary 5: Host ↔ Gemini API (TB5)
*   **Prompt Injection:** Untrusted scraped content from the `Browser` tool alters agent instructions.
    *   **Control (TOOL-BRW-002):** Page content is treated as untrusted data, never executed as instructions.
*   **Information Disclosure:** Long-lived API keys leak into the model context or logs.
    *   **Control (COG-009):** API credentials never enter the model context or agent memory. Tools use short-lived `Ephemeral-Tunnel` credentials.

## Boundary 6: Host ↔ STS (TB6)
*   **Theft:** Credentials requested for a tool call are stolen and reused later.
    *   **Control (TOOL-ET-001, TOOL-ET-003):** STS tokens have very short TTLs tied explicitly to the task duration, and are actively revoked upon task completion or failure.

## Boundary 7: Host ↔ Repository (TB7)
*   **Tampering:** Malicious or malformed edits corrupt the repository state.
    *   **Control (REPO-014, TOOL-FS-002):** Agent-generated CST edits must pass native repository linting hooks (e.g., `prettier`, `ruff`) inside a sandbox before committing. Edits are isolated to separate branches.
