# Architecture Design Specification: Cognitive Cache Saturation Mitigation

## 1. Architecture Discovery
- **Module Ownership**: 
  - `aura-core` (Rust Microkernel): Shadow-tracking daemon for token usage and saturation threshold monitoring.
  - `aura-sdk` (Python Substrate): Metric publisher, and executor of context summarization and session reset logic.
- **Invariants Checked**: 
  - **Zero FFI**: All metrics and control signals pass asynchronously over Zenoh publish/subscribe topics (`tasks/metrics/tokens`, `control/session/compress`).
  - **Docker-Free Sandbox**: N/A, isolation boundaries remain intact.
  - **SPIFFE Identity**: Metrics and control streams will be prefixed and isolated per SVID.

## 2. AST Blast-Radius Pre-Check
- `.agents/knowledge_graph.sqlite` check yields 0 callers (graph isolated/uninitialized). Safe to proceed without manual escalation.

## 3. Substrate Selection
- **Rust Host (Microkernel)**: Asynchronous background `tokio` task (`start_token_monitor`) managing aggregate token limits.
- **Python SDK**: Cognitive orchestration loop handling the summarization inference and cache-invalidation reset.

## 4. Implementation Specification

### Objective
Prevent model context window saturation (and subsequent HTTP 400 errors or performance degradation) by utilizing the Rust microkernel to track token usage and signal the Python agent to compress its context before reaching the hard limit.

### Component 1: Python Telemetry Publisher (`aura_sdk/agent.py`)
- Parse `event.interaction.usage.total_tokens` during the `interaction.completed` SSE event.
- Publish a JSON payload to `tasks/metrics/tokens`:
  ```json
  {
      "agent": "WorkerAgent",
      "session_id": "v1_...",
      "total_tokens": 850000
  }
  ```

### Component 2: Rust Shadow-Tracker (`aura-core/src/transport/zenoh_bus.rs` or `runtime`)
- Spawn a `tokio` subscriber listening on `aura/workspace/<svid>/tasks/metrics/tokens`.
- Maintain a local state cache (e.g., `HashMap<String, u32>`).
- If `total_tokens` exceeds a saturation threshold (e.g., 80% of max context window, configurable threshold), publish a command back to `aura/workspace/<svid>/control/session/compress`.

### Component 3: Python Compression Handler (`aura_sdk/agent.py`)
- The Python agent establishes a Zenoh subscription to `control/session/compress`.
- When triggered, it sets an internal `compression_required` flag.
- Before the next user task or cognitive step, the agent halts normal execution and injects a summarization prompt:
  > "System Context Compression: Summarize the entire conversation history, architectural decisions, and current state into a dense context document."
- The agent captures the generated summary, drops the `previous_interaction_id`, and starts a fresh Interactions API stream, passing the summary as the initial foundational context.

### Verification Gates
1. **Mock Testing**: Emit a mock metric from the test suite with `total_tokens: 999999` and verify the Rust kernel publishes the compression signal.
2. **Context Continuity**: Ensure the resulting session effectively retains key state points despite losing the raw interaction history.

### Rollback Plan
- Revert `aura_sdk/agent.py` and `aura-core` Zenoh subscriptions via `git restore`.
