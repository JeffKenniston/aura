---
command: /grill-me
description: Actively interrogates the developer regarding edge cases, failure modes, and architectural trade-offs.
version: 1.0
---

# /grill-me Workflow: Requirement Interrogation Protocol

When invoked, the agent reverses the questioning loop:

1. **Identify Ambiguities**:
   - Analyze requirements for unstated assumptions in transport fallback, memory bounds, or concurrency limits.

2. **Formulate High-Impact Questions**:
   - Question 1: Failure boundary (e.g. what happens if UDP 443 fails to connect within 1 RTT?).
   - Question 2: Resource bounds (e.g. max fuel limit for WASM components before OutOfFuel trap).
   - Question 3: Security & scoping (e.g. exact path-prefix whitelist for FileSystem tool).

3. **Output**:
   - Present a concise, numbered list of probing questions and offer recommended defaults for each.
