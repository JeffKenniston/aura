---
name: aura-bus-domain
description: Zenoh Event Bus constraints. Activate this skill when working on the
  bus domain.
---
# BUS Domain Rules (Event Bus)
- **Topology**: Host runs as Zenoh router; sandboxes/agents are leaf nodes.
- **Model**: read-switch-execute-return with async completion events.
- **Message Schema**: Include correlation ID, SVID, timestamp, schema version.
- **Security**: mTLS using SVIDs, enforcing key-expression ACLs.
- **Idempotency**: Completion events delivered at-least-once; consumers must be idempotent.
