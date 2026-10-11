---
name: aura-net-domain
description: Transport and MCP constraints. Activate this skill when working on the
  net domain.
---
# NET Domain Rules
- **Protocol**: HTTP/3 over QUIC (primary). HTTP/2 fallback.
- **MCP**: Serve using stateless July 2026 MCP spec.
- **Streaming**: Each tool call maps to its own QUIC stream (no HOL blocking).
- **Security**: TLS 1.3 mandatory; clients authenticate via mTLS (SVID) or OAuth.
