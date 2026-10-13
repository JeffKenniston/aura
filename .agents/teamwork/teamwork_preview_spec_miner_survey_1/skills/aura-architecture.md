# Local copy of aura-architecture
Source: /home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-architecture/SKILL.md

# Aura Architecture Reference
Key Invariants:
- Microkernel Layer Separation: Rust `aura-core` (host, hypervisor, networking, cryptography) vs Python `aura-sdk` (cognitive behaviors).
- Zero Foreign Function Interface (FFI): decoupled via Zenoh event bus.
- Zero-Trust Workload Identity: Dynamic attestation via SPIFFE/SPIRE, default-deny granular authorization, Ephemeral-Tunnel mints short-lived tokens directly without leaking static secrets into LLM contexts.
