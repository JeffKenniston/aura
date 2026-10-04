---
name: spiffe-attestation
description: Verifies SPIFFE workload identity documents (SVIDs), audits zero-trust capability bounds, and validates mTLS communication across agent execution nodes.
version: 1.0
---

# SPIFFE/SPIRE Attestation & Security Verification Skill

## Overview
This skill validates zero-trust cryptographic attestations across Aura's distributed execution nodes. It confirms that agents, WASM plugins, and Firecracker microVMs possess valid, unexpired SVIDs and operate strictly within authorized scopes.

## Workflow Instructions
1. **Inspect Target Workload Identity**:
   - Query the local SPIRE agent socket (`/run/spire/sockets/agent.sock`) for the target workload's SPIFFE ID:
     - Expected format: `spiffe://aura.local/agent/{agent_name}` or `spiffe://aura.local/sandbox/{sandbox_id}`.

2. **Verify Cryptographic SVID**:
   - Validate X.509 certificate chains or JWT-SVID signatures against the Aura Trust Bundle.
   - Verify that the SVID validity window is active and not within expiration thresholds.

3. **Audit Capability Permissions**:
   - Match the requested operation (e.g., FileSystem path write, network socket open) against the SVID's authorized selector attributes.
   - If an unauthorized scope is detected, deny tool execution immediately and log a security audit event to `.agents/audit.log`.
