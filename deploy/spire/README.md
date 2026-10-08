# SPIRE Infrastructure for Aura OS (Development & Testing)

This directory provides the SPIFFE/SPIRE infrastructure required for Zero-Trust workload identity attestation across the Aura OS ecosystem (IAM-001 - IAM-008).

## Architecture Overview

- **Trust Domain**: `aura.local`
- **Workload API Socket**:
  - Containerized: `unix:///tmp/spire-agent/public/api.sock` or `unix://<project-root>/deploy/spire/sockets/agent.sock`
  - Local: `unix://<project-root>/deploy/spire/sockets/agent.sock`
- **Default Workload SVIDs**:
  - Host microkernel: `spiffe://aura.local/host`
  - Default agent: `spiffe://aura.local/agent/default`
  - Sandboxes: `spiffe://aura.local/sandboxes/wasm` and `spiffe://aura.local/sandboxes/firecracker`

---

## Quickstart

### 1. Starting SPIRE

#### Option A: Containerized (Docker Compose)
If Docker is installed on your host:
```bash
./deploy/spire/scripts/start-spire.sh
```
Or directly via Docker Compose:
```bash
cd deploy/spire
docker compose up -d
```

#### Option B: Docker-Free Local SPIRE
Adhering to Aura's Docker-free philosophy, you can run SPIRE natively as local binaries:
```bash
./deploy/spire/scripts/local-spire.sh
```
This downloads verified SPIRE binaries into `deploy/spire/bin/` and starts the server and agent processes locally.

---

### 2. Configuring Aura Core

Configure your environment so `aura-core` connects to the SPIRE Workload API:

```bash
export SPIFFE_ENDPOINT_SOCKET="unix://$(pwd)/deploy/spire/sockets/agent.sock"
```

Verify the workload identity using the SPIRE agent CLI:
```bash
# If using Docker
docker exec spire-agent /opt/spire/bin/spire-agent api fetch x509

# If using local binaries
./deploy/spire/bin/spire-agent api fetch x509 -socketPath ./deploy/spire/sockets/agent.sock
```

---

### 3. Registering Custom Workloads

To register new agent identities with specific capability scopes:
```bash
./deploy/spire/scripts/register-workload.sh spiffe://aura.local/agent/my-agent unix:uid:$(id -u)
```

---

### 4. Stopping SPIRE

```bash
./deploy/spire/scripts/stop-spire.sh
```
