---
title: Gemini Interactions API and Multi-Tiered Cognitive Routing
description: Establishes protocols for server-side state caching, SSE observability, model tier routing, and context saturation mitigation.
tags: [gemini, interactions-api, model-routing, caching, context-management]
version: 1.0
---

# Gemini Interactions API & Multi-Tiered Cognitive Routing

## 1. State Management & Server-Side Caching
- Interactions with foundation models must utilize the Google Gemini Interactions API.
- Use `previous_interaction_id` to leverage server-side caching across iterative turns, optimizing Time-to-First-Token (TTFT) and token spend.
- Observability: Process Server-Sent Events (SSE) detailing model thoughts, tool calls, and execution streams. Function calls are intercepted by the Rust microkernel and injected back into the stream.
- Long-horizon background tasks must use `background=true` with completion tracked via Zenoh-bridged webhooks or polling.

## 2. Multi-Tiered Model Routing Matrix
- **Gemini 3.1 Pro**:
  - Highest intelligence tier. Reserved strictly for complex multi-file architectural planning, high blast-radius refactoring, deep dependency resolution, and multi-step reasoning.
- **Gemini 3.8 Flash**:
  - Default orchestration model for standard software engineering workflows, feature additions, tool orchestrations, and tests.
- **Gemini 3.5 Flash-Lite**:
  - High-throughput, cost-sensitive model for subagent execution, real-time syntax checking, log evaluation, and text classification.
- **Deep Research**:
  - Invoked via `AgentOption` for autonomous data collection, technical paper review, and pre-architecture synthesis.

## 3. Cognitive Cache Saturation Mitigation
- The Rust host maintains a shadow-tracking counter monitoring aggregate session tokens.
- When session token thresholds near context limits, the agent must be paused, a summarization prompt dispatched to distill context, and a new interaction session initialized before cache invalidation.
