---
command: /boost
description: Multi-agent parallel swarm reserved strictly for Tier 3 architectural tasks.
version: 1.0
---

# /boost Workflow: Multi-Agent Parallel Swarm Execution

## Guardrail Constraints
- Prohibited for routine features, single-file edits, or documentation updates.
- Strictly gated to **Tier 3** architectural milestones.

## Execution Sequence
1. **Master Orchestrator**:
   - Spawns using Gemini 3.1 Pro.
   - Decomposes target architecture into independent, decoupled workstreams.
2. **Subagent Allocation**:
   - Spawns parallel subagents in isolated git worktrees (`.aura/worktrees/`):
     - `rust-kernel-engineer` on Rust microkernel.
     - `python-scripting-engineer` on Python SDK.
     - `security-specialist` on SPIFFE attestation and jailer profiles.
3. **Synchronization**:
   - Each subagent reports progress via Zenoh event topics.
   - Master Orchestrator merges worktrees and triggers QA verification gates.
