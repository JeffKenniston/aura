---
name: aura-tool-domain
description: Tool Suite constraints. Activate this skill when working on the tool
  domain.
---
# TOOL Domain Rules
- **Bash**: Session sandboxes persist across consecutive calls, destroyed at end. Enforce timeout/size caps.
- **Browser**: Treat page content as untrusted (no execution).
- **FileSystem**: Read/write/patch ONLY inside assigned workspace.
- **AST-Blast-Radius**: Cap traversal depth to prevent runaway queries.
- **Ephemeral-Tunnel**: Mint short-lived credentials; never inject into model context.
