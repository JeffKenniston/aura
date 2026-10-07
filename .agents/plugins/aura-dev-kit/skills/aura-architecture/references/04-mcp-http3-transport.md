---
title: Model Context Protocol (MCP) over HTTP/3 and QUIC
description: Defines the July 2026 stateless MCP implementation over HTTP/3 with QUIC stream isolation and Alt-Svc fallback.
tags: [mcp, http3, quic, tokio-quiche, transport]
version: 1.0
---

# Model Context Protocol (MCP) over HTTP/3

## Invariant 1: Stateless July 2026 MCP Specification
- Aura tools interface via the stateless Model Context Protocol (July 2026 revision) using HTTP/3 as the default transport.
- Remote MCP server configurations must use the `serverUrl` field. Legacy keys (`url`, `httpUrl`) are deprecated and forbidden.

## Invariant 2: Head-of-Line (HOL) Blocking Elimination via QUIC
- Implemented in the Rust microkernel using `tokio-quiche` with a sans-I/O architecture.
- QUIC manages byte streams independently. Packet drops on one tool invocation stream must not delay or block concurrent tool streams.
- Connection migration and 0-RTT connection resumption via TLS 1.3 connection IDs must be supported to maintain agent sessions across IP/network shifts.

## Invariant 3: Transport Ossification & Silent TCP Fallback
- In enterprise environments where UDP port 443 is blocked or subjected to Deep Packet Inspection (DPI), the transport layer must detect failure within 1 RTT.
- The system must transparently degrade to HTTP/2 over TLS/TCP using `Alt-Svc` headers and ALPN during the handshake without dropping agent requests or throwing runtime errors.
