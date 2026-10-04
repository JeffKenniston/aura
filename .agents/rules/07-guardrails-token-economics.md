---
title: Guardrails, Circuit Breakers, and Token Economics
description: Specifies the Three-Round Rule, tool permission gating, slash commands, and context hygiene.
tags: [guardrails, circuit-breaker, token-economics, permissions]
version: 1.0
---

# Guardrails, Circuit Breakers & Token Economics

## 1. The Three-Round Rule (Circuit Breaker)
- If an agent hits an error, build failure, or verification breakdown, it may attempt up to three autonomous remediation cycles.
- If the issue is not successfully resolved within **three iterations**, the agent must trigger a circuit breaker halt.
- Prohibited action: Endlessly looping with speculative fixes or minor parameter changes.
- Required action: Formulate a precise diagnostic report explaining the failure hypothesis, what was tried, and a targeted question for human review.

## 2. Tool Permission Gating
- The dev environment operates in `proceed-in-sandbox` mode.
- Non-destructive reads, AST inspections, and sandboxed test executions proceed autonomously.
- High-risk operations (destructive host filesystem modifications, outbound network requests outside sandbox boundaries, secret extraction) require explicit manual authorization.

## 3. Slash Command Protocol
- `/plan`: Must be used to force the agent to draft, review, and confirm an implementation plan before writing or modifying project files.
- `/grill-me`: Prompts the agent to actively interrogate the developer concerning architectural edge cases, security implications, and design constraints.
- `/boost`: Multi-agent swarm execution reserved strictly for complex Tier 3 architectural tasks. Strictly prohibited for simple features or bug fixes.

## 4. Context Hygiene
- The agent must respect `.ignore` and `.cursorignore` configurations.
- Never traverse or ingest binary outputs, build directories (`target/`, `dist/`), cache folders (`__pycache__/`, `.cache/`), virtual environments (`.venv/`), or microVM rootfs images.
