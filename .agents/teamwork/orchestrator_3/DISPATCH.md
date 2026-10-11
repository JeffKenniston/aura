## 2026-10-10T04:48:10Z

You are the Project Orchestrator for the Aura Multi-Agent Orchestration Operating System implementation task.

Your working directory is: `/home/jeff/aura/.agents/teamwork/orchestrator_3/`
The authoritative user request is in: `/home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md` (see the section starting at `## 2026-10-10T04:46:40Z`).
Reference documents:
- `/home/jeff/aura/M0_M3_Audit_Report.md` (specifically the 'Gap Analysis Summary & Concrete Actionable Next Steps' section outlining the 11 partially implemented requirements)
- `/home/jeff/aura/requirements.md`

Your mission:
Implement the 11 partially implemented requirements outlined in the 'Gap Analysis Summary & Concrete Actionable Next Steps' section of the M0-M3 Audit Report for the Aura Multi-Agent Orchestration Operating System. Ensure a production-grade implementation.
Integrity mode: demo
Working directory: /home/jeff/aura

Core Requirements:
1. R1. Rust Microkernel Runtime & Concurrency Improvements: Configure a dedicated multi-threaded Tokio runtime pool with separate blocking task handling, runtime health metrics, and implement an empirical load test asserting 5,000+ concurrent QUIC streams without starvation.
2. R2. Declarative Configurations & MCP Integration: Implement TOML manifest loading for the Component Registry via Serde, and build a stateless Model Context Protocol (MCP) server interface over HTTP/3 that handles JSON-RPC 2.0 `tools/list` and `tools/call` requests.
3. R3. Authentic Task Execution & MicroVM Sandboxing: Replace the simulated task execution in the Zenoh bus listener with real WASM and Firecracker microVM dispatch (using dynamic SVIDs). Implement `/dev/kvm` nested virtualization validation and ensure guest filesystems use immutable rootfs layering (e.g. via overlayfs or scratch disks).
4. R4. Persistent Stateful Tools: Build a persistent AF_VSOCK shell daemon for bash session state continuity, and wire a host-to-guest vsock TCP forwarder to connect an async Python CDP client for the headless browser tool.
5. R5. Architecture Documentation (GAP-003): Acknowledge the WASI 0.3 dynamic linking limitation (GAP-003) by explicitly maintaining the current static WIT linking mitigation and adding appropriate documentation/comments.

Acceptance Criteria:
Rust Core Tests:
- `cargo test -p aura-core` passes successfully, including new unit tests for the Tokio runtime thread allocation and metric tracking.
- A new empirical load test (`http3_stream_concurrency_test.rs`) successfully simulates and asserts 5,000+ concurrent client QUIC streams completing round-trips without socket exhaustion.
- A new registry load test successfully parses a valid TOML file defining both WASM and Firecracker tool backends.
- `/dev/kvm` validation tests correctly propagate errors when the device is missing or inaccessible.

Execution & Transport Validation:
- Integration tests verify that Zenoh `tasks/dispatch` messages invoke the actual sandbox backends and return real stdout/stderr/exit codes on `tasks/completion`.
- Integration tests verify that the new MCP HTTP/3 endpoint successfully handles `tools/list` and `tools/call` requests.
- Writes to the rootfs in a Firecracker microVM do not mutate the host's golden disk image across successive boots.

Stateful Tools Tests:
- Bash tool invocations for the same session ID persist environment variables and working directory state across sequential calls.
- The Python SDK `BrowserTool` successfully connects to Chromium's CDP port 9222 via vsock forwarding and can execute basic browser automation actions.
- `pytest aura-sdk/tests` passes successfully.

Repository Rules:
- Verification Gates: Code must be production-ready. ABSOLUTELY NO mocks, stubs, or TODOs.
- Write metadata only to your working directory (`/home/jeff/aura/.agents/teamwork/orchestrator_3/`). Do not write metadata files outside `.agents/teamwork/`. Never create `AGENTS.md` or `GEMINI.md`.
- Maintain `plan.md` and `progress.md` in your working directory.
- Dispatch specialists as needed, verify all changes against the acceptance criteria, and notify when complete.
