# EXE-VM Domain Rules
- **Engine**: Firecracker microVMs through KVM.
- **Isolation**: Launch via jailer with cgroup v2, chroot, namespaces, seccomp.
- **Lifecycle**: Session-scoped. Destroyed at session end.
- **Filesystem**: Read-only base images + ephemeral writable overlay.
- **Networking**: Denied by default; allow-listed per tool/scope.
- **Egress**: Forced through host-level transparent TLS proxy.
