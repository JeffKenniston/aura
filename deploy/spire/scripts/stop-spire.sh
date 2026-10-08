#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SPIRE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

echo "=== Stopping SPIRE Infrastructure ==="

if command -v docker >/dev/null 2>&1; then
    cd "${SPIRE_DIR}"
    if docker compose version >/dev/null 2>&1; then
        DOCKER_COMPOSE="docker compose"
    else
        DOCKER_COMPOSE="docker-compose"
    fi
    ${DOCKER_COMPOSE} down -v || true
fi

# Stop any local running SPIRE instances
pkill -f "spire-agent" 2>/dev/null || true
pkill -f "spire-server" 2>/dev/null || true

# Clean up sockets
rm -f "${SPIRE_DIR}/sockets/agent.sock" 2>/dev/null || true
rm -f /tmp/spire-agent/public/api.sock 2>/dev/null || true

echo "SPIRE stopped successfully."
