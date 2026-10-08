#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -lt 2 ]; then
    echo "Usage: $0 <spiffe_id> <selector_1> [selector_2 ...]"
    echo "Example: $0 spiffe://aura.local/agent/coder unix:uid:1000"
    exit 1
fi

SPIFFE_ID="$1"
shift

SELECTORS=""
for sel in "$@"; do
    SELECTORS="${SELECTORS} -selector ${sel}"
done

echo "Registering Workload Entry:"
echo "  SPIFFE ID: ${SPIFFE_ID}"
echo "  Selectors: $*"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SPIRE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

if command -v docker >/dev/null 2>&1 && docker ps | grep -q spire-server; then
    docker exec spire-server /opt/spire/bin/spire-server entry create \
        -spiffeID "${SPIFFE_ID}" \
        -parentID "spiffe://aura.local/spire/agent/join_token" \
        ${SELECTORS}
elif [ -x "${SPIRE_DIR}/bin/spire-server" ]; then
    "${SPIRE_DIR}/bin/spire-server" entry create \
        -config "${SPIRE_DIR}/server/server.conf" \
        -spiffeID "${SPIFFE_ID}" \
        -parentID "spiffe://aura.local/spire/agent/join_token" \
        ${SELECTORS}
else
    echo "Error: SPIRE server is not running or spire-server binary not found."
    exit 1
fi

echo "Workload registered successfully."
