## 2025-02-18 - [aura-core] Offloading blocking operations in Zenoh bus tools listener

**Learning:** When handling Zenoh messages that dispatch synchronous blocking logic (such as hypervisor process execution, file I/O operations, or external tool execution), failing to move the blocking code out of the async block causes Tokio thread starvation. The event listener halts processing incoming messages on that worker until the blocking code finishes.

**Action:** Wrap blocking tool dispatches within `tokio::task::spawn_blocking` and structure the handler as `tokio::spawn(async move { ... })` so that the main listener continues quickly polling `recv_async()` without interrupting message ingestion.
