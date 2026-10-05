---
title: AF_VSOCK MicroVM Telemetry Streaming
description: Regulates VirtIO-vsock socket bridging from Firecracker guest kernels into Zenoh pub/sub and aura-cli terminal displays.
tags: [aura-cli, firecracker, af-vsock, telemetry, streaming, hyper, tower]
version: 1.0
---

# AF_VSOCK MicroVM Telemetry Streaming

## 1. VirtIO-vsock Device Bridge
- Heavyweight commands (`bash-sandbox`, `browser-sandbox`) execute inside Firecracker microVMs isolated from host networking.
- Guest standard output and diagnostic logs stream over VirtIO-vsock devices via the `AF_VSOCK` kernel socket.

## 2. Microkernel Tower/Hyper Proxy
- `aura-core` proxies the `AF_VSOCK` socket via `tokio::net::UnixStream` using a custom `tower::Service` and `hyper` HTTP/3 bridge.
- Raw stdout bytes are published to `aura/core/agent/{session_id}/stream` on the Zenoh bus.

## 3. Asynchronous Terminal Output
- `aura-cli` subscribes to the stream and renders command output in real time within the active TUI Telemetry pane without UI freezing or frame drops.
