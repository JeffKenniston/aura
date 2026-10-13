## 2026-10-11T14:44:00Z
You are orchestrator_10, the Project Orchestrator for Phase M5 (Hardening) of the Aura Multi-Agent Orchestration Operating System.

Your working directory is: /home/jeff/aura/.agents/teamwork/orchestrator_10
The repository workspace is: /home/jeff/aura
The authoritative request is recorded in: /home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md

## Mission & Requirements
Implement Phase M5 (Hardening) of the Aura Multi-Agent Orchestration Operating System, delivering production-grade Ephemeral-Tunnel credential isolation with VCS PR integration, Computer/GUI Firecracker microVM automation with screenshot redaction hooks, and microkernel zero-trust security hardening (vsock stream buffer truncation, egress proxy enforcement, SPIRE trust federation, and AES-256 encrypted persistence).

### R1. Ephemeral Credential Lifecycle & Out-of-Band Secret Isolation
The system must provide an Ephemeral-Tunnel capability that mints short-lived, task-scoped credentials and injects them out-of-band directly into target execution sandboxes without exposing raw secrets to the model context or agent memory. Injected credentials must be automatically revoked upon task completion, cancellation, or failure, and must support end-to-end pull request creation with version control systems without granting ambient authority.

### R2. Sandboxed GUI Automation & Visual Inspection with Redaction
The system must support a Computer GUI tool running within an isolated Firecracker microVM that receives synthetic user interactions (mouse clicks, moves, drags, keyboard text input, and key presses) and captures PNG screen frames over vsock. Screenshots returned to the orchestrator or stored as persistent artifacts must pass through customizable redaction filters that detect and mask sensitive visual regions before presentation to model context.

### R3. Microkernel Zero-Trust Security & Storage Encryption
The execution host must enforce zero-trust isolation boundaries across all sandboxes: microVM vsock streams must enforce strict memory buffer limits that instantly truncate rogue payloads on violation, and microVM network egress must be restricted exclusively to allow-listed package registries through transparent host-level TLS enforcement. Furthermore, persistent runtime state, task ledgers, and database storage must be encrypted at rest using authenticated AES-256-GCM tied to attested SPIFFE identity keys.

### R4. SPIRE Identity Federation & Transport Fallback Resilience
The identity subsystem must support trust domain federation allowing workloads to authenticate seamlessly against external enterprise OIDC and identity providers without privilege escalation. The network transport layer must detect UDP/QUIC connection failures within one round-trip time and fail over silently to HTTP/2 over TCP without dropping in-flight tool calls.

## Acceptance Criteria
- Ephemeral credentials minted with explicit TTLs return opaque, sanitized handles (`[REDACTED-TOKEN-...]`) whenever inspected by prompt/logging formatters.
- Attempting to mint credentials outside the caller's authorized SPIFFE scopes or unlisted services returns an authorization denial error.
- Out-of-band credential injection successfully establishes sandbox credentials, and explicit revocation invalidates the credential immediately across all sessions.
- Automated PR operations through `GitManager::open_pr` use ephemeral tokens and cleanly revoke credentials upon completion or error.
- The guest vsock daemon receives and acknowledges JSON-RPC input actions (`click`, `move`, `type`, `key`, `drag`) in the X11 virtual display environment without dropping events.
- The Computer tool captures valid PNG screenshots and applies host-side redaction masks over specified sensitive bounding boxes/patterns before storing artifacts.
- The Python SDK `ComputerTool` exposes async methods for mouse actions, typing, and redacted screenshot capture with 100% passing unit tests.
- Vsock stream listeners enforce a configurable memory buffer limit and instantly truncate incoming streams that exceed the limit without panicking or leaking memory.
- MicroVM egress through the transparent TLS proxy blocks unauthorized external domains with TLS fatal alerts while passing allow-listed package mirrors.
- SQLite database files and task ledgers at rest are encrypted with AES-256-GCM; raw disk byte inspection confirms ciphertext with no plaintext schema or task strings.
- Workload identity verification authenticates federated trust domains and enterprise OIDC tokens according to SPIRE federation mappings.
- Transport connection initialization seamlessly falls back from HTTP/3 to HTTP/2 TCP within 1 RTT when UDP traffic is blocked.

## Verification Gates
- `cargo test -p aura-core --jobs 1` passes with zero failures and zero regressions.
- `cargo test --manifest-path tools/ephemeral-tunnel/Cargo.toml` and `cargo test --manifest-path deploy/images/vsock-screenshot-daemon/Cargo.toml` pass cleanly.
- `/home/jeff/aura/aura-sdk/.venv/bin/pytest aura-sdk/tests` passes all tests.
- `/home/jeff/aura/aura-sdk/.venv/bin/ruff check aura-sdk/aura_sdk aura-sdk/tests` passes with zero lint violations.
- `git status` reports a clean working tree with no untracked temporary build files.
- Constitution: ABSOLUTELY NO mocks, stubs, or TODOs. Code must be production-ready.


## 2026-10-11T15:37:08Z
Server restarted. Reviving teamwork preview orchestration for Phase M5 (Hardening).
Please check on the status of your subagents (`worker_m1_1`, `test_writer_e2e_1`), revive or re-dispatch workers as needed, and continue driving Milestone 1 through completion, followed by Milestones 2-4 and the final verification gates.
