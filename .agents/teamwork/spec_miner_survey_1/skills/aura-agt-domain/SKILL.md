---
name: aura-agt-domain
description: Cognitive Scripting SDK constraints. Activate this skill when working
  on the agt domain.
---
# AGT Domain Rules (Cognitive Scripting)
- **Language**: Python / TypeScript
- **Isolation**: Each agent runs in its own OS process. Communication with host ONLY via Zenoh bus.
- **Async Primitives**: call_tool, dispatch_background, await_event, emit.
- **Identity**: Obtain SVID from SPIRE workload API at start-up; attach to every request.
- **Dependencies**: Pin and isolate per agent.
- **State**: Use state-checkpointing decorators for long-running tasks.
