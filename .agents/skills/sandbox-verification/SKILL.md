---
name: sandbox-verification
description: Audits and validates the runtime isolation parameters of Firecracker microVMs and WASI 0.3 sandboxes in the WSL development environment.
version: 1.0
---

# Sandbox Isolation & Runtime Verification Skill

## Overview
This skill performs pre-flight checks and isolation verification on the execution sandboxes in the local WSL2 environment. It ensures KVM hardware virtualization is accessible, jailer filters are intact, and fuel limits are enforced.

## Workflow Instructions
1. **WSL KVM Hardware Acceleration Check**:
   - Verify `/dev/kvm` exists and the user has read/write permissions (`chmod 666 /dev/kvm` or appropriate group membership).
   - Ensure nested virtualization is enabled in the WSL2 configuration (`.wslconfig` `nestedVirtualization=true`).

2. **Firecracker & Jailer Validation**:
   - Verify the `firecracker` and `jailer` binaries are installed and accessible in the system path.
   - Confirm cgroup v2 controller availability (`/sys/fs/cgroup`).
   - Validate that default seccomp profiles restrict dangerous syscalls (`ptrace`, `reboot`, `kexec_load`).

3. **WASI 0.3 Fuel-Metering Verification**:
   - Test `wasmtime` fuel consumption against a standard loop.
   - Confirm execution terminates deterministically with an `OutOfFuel` trap when instruction limits are exceeded.
