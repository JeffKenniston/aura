---
name: cedar-policy-verification
description: Verifies Cedar policy schemas, validates permission tiers (strict, request-review, proceed-in-sandbox, always-proceed), and audits progressive authorization decisions.
version: 1.0
---

# Cedar Policy Verification Skill

## Overview
This skill validates that `aura-cli`'s access control rules adhere strictly to the Cedar policy specification (`aura.cedarschema` and `default.cedar`), ensuring mutating tool calls are gated behind the correct progressive authorization tier.

## Verification Procedures
1. **Validate Cedar Schema**:
   - Verify entity types: `Aura::Workload`, `Aura::Action`, `Aura::Workspace`.
   - Validate action permissions: `ReadFile`, `QueryIndex`, `WriteFile`, `ExecuteBash`.

2. **Test Permission Tiers**:
   - **strict**: Confirm all actions except ReadFile require explicit terminal confirmation.
   - **request-review**: Confirm ReadFile and QueryIndex proceed automatically; WriteFile and ExecuteBash halt for diff review.
   - **proceed-in-sandbox**: Confirm execution proceeds automatically inside Firecracker microVMs while host writes are blocked.
   - **always-proceed**: Confirm full autonomy without prompting in automated environments.
