---
title: Two-Layer Zero-Trust Security and Cedar Progressive Authorization
description: Defines SPIFFE/SPIRE mTLS transport attestation and Cedar policy progressive authorization tiers for aura-cli.
tags: [aura-cli, security, spiffe, spire, mtls, cedar, authorization]
version: 1.0
---

# Two-Layer Zero-Trust Security & Cedar Progressive Authorization

## Layer 1: Transport Attestation (SPIFFE / SPIRE)
- `aura-cli` connects to the local SPIRE agent endpoint via `WorkloadApiClient::connect_env()`.
- Validates executable hash, UID, and GID to acquire an X.509 SVID bound to `spiffe://aura.local/workload/aura-cli`.
- Wraps the Zenoh IPC session in mutual TLS (`spiffe-rustls-tokio`). Connections without valid cluster SVIDs are immediately dropped.

## Layer 2: Progressive Authorization (Cedar Policy Engine)
- All tool execution requests are validated through an embedded Cedar policy engine (`cedar-policy`, `cedarling`) before execution.
- Four Progressive Authorization Tiers:
  1. `strict`: Zero-trust read-only enforcement. All file writes, network calls, and command executions require explicit terminal confirmation.
  2. `request-review` (Default): High autonomy for planning, AST search, and diff generation. The UI halts execution at mutations and prompts the user with an interactive diff pane.
  3. `proceed-in-sandbox`: Autonomous tool execution routed into isolated Firecracker microVMs or WASI engines without prompting. Direct host writes are blocked.
  4. `always-proceed`: Direct host mutation without prompting. Strictly restricted to automated CI/CD pipelines.

## Policy Schema Enforcement
- Cedar policies must follow `aura.cedarschema`, defining principals (`Aura::Workload`), actions (`Aura::Action`), and resources (`Aura::Workspace`).
