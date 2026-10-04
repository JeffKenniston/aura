---
name: circuit-breaker
description: Implements the Three-Round Rule diagnostic recovery workflow, halting autonomous loops and formulating clarifying questions when failures persist.
version: 1.0
---

# Three-Round Rule Circuit Breaker Skill

## Overview
This skill operationalizes the Three-Round Rule invariant. When an agent experiences three consecutive failed attempts to resolve an error, compile, or satisfy a verification gate, this skill halts execution and initiates human escalation.

## Workflow Instructions
1. **Track Iteration History**:
   - Record the root cause, attempted patch, and resulting error message across turns 1, 2, and 3.

2. **Trigger Autonomous Halt**:
   - Immediately cease further automated edits or tool retries.
   - Do not attempt speculative variants or repeated brute-force recompilation.

3. **Synthesize Diagnostic Dossier**:
   - Document:
     - **Goal**: What the agent was attempting to achieve.
     - **Attempts 1-3**: Concise summary of each hypothesis and why it failed.
     - **Current State**: Exact failure output or compiler trace.
     - **Blocker**: The exact ambiguity or system constraint causing the deadlock.

4. **Formulate Clarifying Question**:
   - Present the diagnostic dossier to the developer accompanied by a single, direct question offering viable paths forward.
