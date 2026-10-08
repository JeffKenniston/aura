# ADR 002: Identity, Cognitive Routing, and Repository Intelligence

## Status
Accepted

## Context
Aura requires a Zero-Trust architecture using SPIFFE/SPIRE, dynamic cognitive model routing, safe handling of concurrent repository modifications, and protections against runaway context windows.

## Decisions
1. **IAM / Identity Issuance (IAM):**
   - The Phase 1 design will focus strictly on **core local SPIRE integration** (local SVID issuance and validation). Enterprise trust domain federation (IAM-008) will be handled as a fast-follow design document.
2. **Model Router Heuristics (RTE):**
   - The router will determine the model tier (Pro, Flash, Flash-Lite, Deep Research) using a **rule-based heuristic engine**. This engine will evaluate the AST blast radius of the proposed changes, the prompt token count, and historical task success rates.
3. **Repository Conflict Resolution (REPO):**
   - When a concurrent human commit is detected before a patch applies, the host will **reject the patch** and send the conflict details back to the agent as a tool-call error. The agent is responsible for re-evaluating the new CST and generating a fresh patch.
4. **Context Window Mitigation (GAP):**
   - To prevent runaway infinite loops when context windows reach 80% capacity, the shadow tracker will enforce a **hard limit of 5 consecutive summarization loops**. If this limit is exceeded, the task will be aborted.

## Consequences
- Reduces Phase 1 IAM complexity by deferring federation.
- Rule-based routing keeps the router deterministic and avoids the overhead of managing a local ML classifier.
- Forcing agents to resolve git conflicts simplifies host logic but increases the cognitive burden on the agent.
- A 5-loop limit prevents catastrophic token spend on stalled tasks.
