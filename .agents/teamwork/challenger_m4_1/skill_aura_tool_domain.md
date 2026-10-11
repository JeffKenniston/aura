# Local Copy: aura-tool-domain SKILL.md
Source: /home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-tool-domain/SKILL.md

# TOOL Domain Rules
- **Bash**: Session sandboxes persist across consecutive calls, destroyed at end. Enforce timeout/size caps.
- **Browser**: Treat page content as untrusted (no execution).
- **FileSystem**: Read/write/patch ONLY inside assigned workspace.
- **AST-Blast-Radius**: Cap traversal depth to prevent runaway queries.
- **Ephemeral-Tunnel**: Mint short-lived credentials; never inject into model context.
