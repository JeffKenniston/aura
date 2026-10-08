# ADR 001: Execution Host Sandboxing and Orchestration

## Status
Accepted

## Context
The Aura execution host needs to run agent tool calls in isolated sandboxes without using Docker or OCI containers. The workloads are split into lightweight (computational) and heavyweight (OS integrations). We need to decide on the VM lifecycle and the internal Rust host orchestration model to manage these sandboxes asynchronously via the Zenoh event bus.

## Decisions
1. **Sandboxing Strategy:** 
   - Use WASI 0.3 WebAssembly components for lightweight tasks.
   - Use Firecracker microVMs for heavyweight tasks.
2. **MicroVM Lifecycle (EXE-VM):**
   - We will design for **cold boots only** initially. With Firecracker + KVM, cold boot times under 125ms are achievable and sufficient for Phase 1. Snapshot pooling and recycle mechanisms are deferred as a future optimization.
3. **Host Orchestration Concurrency:**
   - The Rust host will use **standard Tokio asynchronous tasks (`tokio::spawn`)** mapping directly to Zenoh subscriptions. We will avoid heavyweight actor frameworks (like `actix`) and manage state via standard channels/mutexes.
4. **Event Bus Backpressure (BUS):**
   - The host will buffer tool-call events in-memory up to a configurable limit. Once the limit is reached, it will apply backpressure by rejecting new publications from the agent with a `RateLimitExceeded` error.

## Consequences
- Simplifies Phase 1 implementation by avoiding complex snapshot management.
- Standard Tokio tasks provide high performance with minimal framework overhead.
- Agents must be designed to gracefully handle `RateLimitExceeded` backpressure errors during bursts of tool calls.
