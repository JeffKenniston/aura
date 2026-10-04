# Architectural Implementation Specification: Long-Horizon Background Orchestration

## 1. Architecture Discovery
- **Module Ownership**:
  - `aura-sdk` (Python Substrate): The `agent.py` module responsible for API requests must support the `background=True` parameter in the Gemini Interactions API.
  - `aura-core` (Rust Microkernel): To avoid blocking or high-frequency polling, a Zenoh-bridged webhook endpoint needs to be exposed by the Rust host to receive HTTP POST callbacks from the API when the background interaction status changes.
- **Invariants Checked**:
  - **Zero FFI**: Webhook payloads received by Rust (`aura-core`) will be published over the Zenoh bus, and Python (`aura-sdk`) will subscribe to the event. No direct FFI.
  - **Docker-Free Execution**: The background interaction executes autonomously in the cloud API, while our webhook bridge runs on the Rust host.

## 2. AST Blast-Radius Pre-Check
- **Target Symbol**: `execute_task` in `aura-sdk/aura_sdk/agent.py` and `ZenohClient` in `aura-sdk/aura_sdk/transport.py`.
- **Query Results**: 2 upstream callers (found in `tests/test_cognitive_cache.py` and `tests/test_cognitive_loop.py`).
- **Blast Radius**: Low (< 3 callers). Escalate to Gemini 3.1 Pro (Already assigned).

## 3. Substrate Selection
- **Role**: Cognitive orchestration layer and network bridging.
- **Substrate**: The API interaction logic executes in the host Python environment via `aura-sdk`, while the webhook listener operates directly within the `aura-core` Tokio asynchronous runtime.

## 4. Implementation Details

### Background Execution Parameter
- **API Call Modification**: Update `execute_task` (or create a dedicated `execute_background_task`) in `aura_sdk/agent.py` to pass `background=True` into `self.client.aio.interactions.create()`.
- **Interaction Management**: The API will immediately return an `Interaction` object with an `id` and status `in_progress` without streaming blocking events.

### Zenoh-Bridged Webhook Monitoring
- **Rust Webhook Listener (`aura-core`)**:
  - Implement a lightweight HTTP listener in `aura-core` running on an exposed port.
  - When the Gemini API sends an event to the webhook, Rust deserializes the status update.
  - `aura-core` publishes the status payload to the Zenoh bus under `aura/workspace/{svid}/interactions/{interaction_id}/status`.
- **Python Substrate Subscriber (`aura-sdk`)**:
  - The `ZenohClient` in `aura-sdk` will declare a subscriber for `interactions/+/status`.
  - When a `completed`, `failed`, or `requires_action` event arrives over Zenoh, the Python orchestrator resumes execution, retrieves the final result via `self.client.aio.interactions.get()`, and proceeds without aggressively polling in an active loop.

## 5. Verification Gates
1. **Lint and Type Check**: `cargo clippy`, `cargo fmt --check`, `ruff`, and `mypy` must pass.
2. **End-to-End Tests**:
   - Write a mock test verifying that invoking a task with `background=True` returns immediately.
   - Simulate an HTTP POST to the Rust webhook endpoint and assert that the Python agent receives the Zenoh callback and transitions state successfully.

## 6. Rollback Plan
- Revert changes to `aura-sdk/aura_sdk/agent.py` and `transport.py`.
- Revert additions in `aura-core` for the webhook listener.
