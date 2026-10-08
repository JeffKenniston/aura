#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SPIRE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

echo "=== Starting SPIRE Infrastructure for Aura OS ==="

mkdir -p "${SPIRE_DIR}/data/server"
mkdir -p "${SPIRE_DIR}/data/agent"
mkdir -p "${SPIRE_DIR}/sockets"
mkdir -p /tmp/spire-agent/public 2>/dev/null || true

# Check if Docker is installed
if command -v docker >/dev/null 2>&1; then
    echo "Starting SPIRE via Docker Compose..."
    cd "${SPIRE_DIR}"
    
    if docker compose version >/dev/null 2>&1; then
        DOCKER_COMPOSE="docker compose"
    else
        DOCKER_COMPOSE="docker-compose"
    fi

    ${DOCKER_COMPOSE} up -d

    echo "Waiting for SPIRE Server to be healthy..."
    until docker exec spire-server /opt/spire/bin/spire-server healthcheck >/dev/null 2>&1; do
        sleep 1
    done
    echo "SPIRE Server is healthy."

    # Register default Aura host workload entry
    CURRENT_UID="$(id -u)"
    echo "Registering default Aura host workload entry for UID ${CURRENT_UID}..."
    docker exec spire-server /opt/spire/bin/spire-server entry create \
        -spiffeID "spiffe://aura.local/host" \
        -parentID "spiffe://aura.local/spire/agent/join_token" \
        -selector "unix:uid:${CURRENT_UID}" || true

    # Register default agent workload entry
    docker exec spire-server /opt/spire/bin/spire-server entry create \
        -spiffeID "spiffe://aura.local/agent/default" \
        -parentID "spiffe://aura.local/spire/agent/join_token" \
        -selector "unix:user:$(whoami)" || true

    # Register sandbox workload entry
    docker exec spire-server /opt/spire/bin/spire-server entry create \
        -spiffeID "spiffe://aura.local/sandboxes/wasm" \
        -parentID "spiffe://aura.local/spire/agent/join_token" \
        -selector "unix:uid:${CURRENT_UID}" || true

    echo ""
    echo "=== SPIRE Environment Ready ==="
    echo "Workload API socket is available at: ${SPIRE_DIR}/sockets/agent.sock"
    echo "Run the following in your terminal to set the endpoint:"
    echo "  export SPIFFE_ENDPOINT_SOCKET=\"unix://${SPIRE_DIR}/sockets/agent.sock\""
    echo "Or for system default:"
    echo "  export SPIFFE_ENDPOINT_SOCKET=\"unix:///tmp/spire-agent/public/api.sock\""
else
    echo "Docker is not found in PATH."
    echo "Aura supports Docker-free local development via ./scripts/local-spire.sh."
    echo "Running local SPIRE runner..."
    exec "${SCRIPT_DIR}/local-spire.sh"
fi
