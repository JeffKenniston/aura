#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SPIRE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
BIN_DIR="${SPIRE_DIR}/bin"
SPIRE_VERSION="1.8.7"

mkdir -p "${BIN_DIR}"
mkdir -p "${SPIRE_DIR}/data/server"
mkdir -p "${SPIRE_DIR}/data/agent"
mkdir -p "${SPIRE_DIR}/sockets"
mkdir -p /tmp/spire-agent/public 2>/dev/null || true

# Download SPIRE binaries if not present
if [ ! -x "${BIN_DIR}/spire-server" ] || [ ! -x "${BIN_DIR}/spire-agent" ]; then
    echo "Downloading SPIRE ${SPIRE_VERSION} binaries for Docker-free local execution..."
    ARCH="$(uname -m)"
    case "${ARCH}" in
        x86_64) SPIRE_ARCH="amd64" ;;
        aarch64|arm64) SPIRE_ARCH="arm64" ;;
        *) echo "Unsupported architecture: ${ARCH}"; exit 1 ;;
    esac

    TARBALL="spire-${SPIRE_VERSION}-linux-${SPIRE_ARCH}-musl.tar.gz"
    URL="https://github.com/spiffe/spire/releases/download/v${SPIRE_VERSION}/${TARBALL}"
    
    TMP_DIR="$(mktemp -d)"
    echo "Fetching ${URL}..."
    if command -v curl >/dev/null 2>&1; then
        curl -sSL "${URL}" -o "${TMP_DIR}/${TARBALL}"
    elif command -v wget >/dev/null 2>&1; then
        wget -q "${URL}" -O "${TMP_DIR}/${TARBALL}"
    else
        echo "Error: Neither curl nor wget found."
        exit 1
    fi

    tar -xzf "${TMP_DIR}/${TARBALL}" -C "${TMP_DIR}"
    cp "${TMP_DIR}/spire-${SPIRE_VERSION}/bin/spire-server" "${BIN_DIR}/"
    cp "${TMP_DIR}/spire-${SPIRE_VERSION}/bin/spire-agent" "${BIN_DIR}/"
    rm -rf "${TMP_DIR}"
    chmod +x "${BIN_DIR}/spire-server" "${BIN_DIR}/spire-agent"
    echo "SPIRE binaries installed to ${BIN_DIR}."
fi

# Generate local server config
LOCAL_SERVER_CONF="${SPIRE_DIR}/data/server/local_server.conf"
cat > "${LOCAL_SERVER_CONF}" <<EOF
server {
    bind_address = "127.0.0.1"
    bind_port = "8081"
    trust_domain = "aura.local"
    data_dir = "${SPIRE_DIR}/data/server"
    log_level = "INFO"
    ca_key_type = "rsa-2048"
    ca_ttl = "168h"
    default_x509_svid_ttl = "1h"
    default_jwt_svid_ttl = "15m"
}

plugins {
    DataStore "sql" {
        plugin_data {
            database_type = "sqlite3"
            connection_string = "${SPIRE_DIR}/data/server/datastore.sqlite3"
        }
    }
    NodeAttestor "join_token" {
        plugin_data {}
    }
    KeyManager "disk" {
        plugin_data {
            keys_path = "${SPIRE_DIR}/data/server/keys.json"
        }
    }
}
EOF

# Generate local agent config
LOCAL_AGENT_CONF="${SPIRE_DIR}/data/agent/local_agent.conf"
AGENT_SOCKET="${SPIRE_DIR}/sockets/agent.sock"
cat > "${LOCAL_AGENT_CONF}" <<EOF
agent {
    data_dir = "${SPIRE_DIR}/data/agent"
    log_level = "INFO"
    server_address = "127.0.0.1"
    server_port = "8081"
    socket_path = "${AGENT_SOCKET}"
    trust_domain = "aura.local"
}

plugins {
    NodeAttestor "join_token" {
        plugin_data {}
    }
    KeyManager "disk" {
        plugin_data {
            directory = "${SPIRE_DIR}/data/agent"
        }
    }
    WorkloadAttestor "unix" {
        plugin_data {
            discover_workload_path = true
        }
    }
}
EOF

# Start spire-server
echo "Starting local SPIRE Server..."
"${BIN_DIR}/spire-server" run -config "${LOCAL_SERVER_CONF}" > "${SPIRE_DIR}/data/server/server.log" 2>&1 &
SERVER_PID=$!
echo "SPIRE Server started (PID: ${SERVER_PID})."

sleep 2

# Generate join token
echo "Generating node join token..."
TOKEN_OUTPUT=$("${BIN_DIR}/spire-server" token generate -config "${LOCAL_SERVER_CONF}" -spiffeID "spiffe://aura.local/agent/local_node")
JOIN_TOKEN=$(echo "${TOKEN_OUTPUT}" | awk '/Token:/ {print $2}')

# Start spire-agent
echo "Starting local SPIRE Agent..."
"${BIN_DIR}/spire-agent" run -config "${LOCAL_AGENT_CONF}" -joinToken "${JOIN_TOKEN}" > "${SPIRE_DIR}/data/agent/agent.log" 2>&1 &
AGENT_PID=$!
echo "SPIRE Agent started (PID: ${AGENT_PID})."

sleep 3

# Register default workloads
CURRENT_UID="$(id -u)"
echo "Registering default Aura host workload for UID ${CURRENT_UID}..."
"${BIN_DIR}/spire-server" entry create \
    -config "${LOCAL_SERVER_CONF}" \
    -spiffeID "spiffe://aura.local/host" \
    -parentID "spiffe://aura.local/agent/local_node" \
    -selector "unix:uid:${CURRENT_UID}" || true

"${BIN_DIR}/spire-server" entry create \
    -config "${LOCAL_SERVER_CONF}" \
    -spiffeID "spiffe://aura.local/agent/default" \
    -parentID "spiffe://aura.local/agent/local_node" \
    -selector "unix:user:$(whoami)" || true

echo ""
echo "=== Local SPIRE Ready ==="
echo "Workload API socket: ${AGENT_SOCKET}"
echo "Run the following in your shell:"
echo "  export SPIFFE_ENDPOINT_SOCKET=\"unix://${AGENT_SOCKET}\""
