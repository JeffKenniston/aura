---
title: Zero-Trust Workload Identity and SPIFFE/SPIRE Attestation
description: Mandates dynamic SVID attestation and default-deny authorization across all agents and sandboxes.
tags: [security, spiffe, spire, svid, zero-trust]
version: 1.0
---

# Zero-Trust Workload Identity

## Invariant 1: Dynamic Attestation via SPIFFE/SPIRE
- Every active agent process, WASM component instance, and Firecracker microVM must be dynamically attested via the SPIRE runtime upon instantiation.
- Workloads receive a cryptographically signed SPIFFE Verifiable Identity Document (SVID) encoding their specific identity string (e.g., `spiffe://aura.local/agent/<agent-id>`).

## Invariant 2: Default-Deny Granular Authorization
- No agent or sandbox may invoke an MCP tool or OS capability without presenting an active, valid SVID authorizing that explicit capability scope.
- Filesystem tool invocations require an SVID validated against strict path-prefix boundaries.
- Ephemeral credential tools (`Ephemeral-Tunnel`) must communicate directly with the SPIRE server to mint short-lived tokens (e.g., AWS STS, Vault tokens) valid only for the task duration, keeping static secrets completely out of LLM contexts.
- Compromised or unauthenticated workloads are denied execution and immediately isolated.
