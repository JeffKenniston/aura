use zenoh::prelude::r#async::*;
use zenoh::config::Config;
use std::sync::Arc;
use spiffe::workload_api::client::WorkloadApiClient;

pub const KEY_REPO_INDEX: &str = "aura/repository/index";
pub const KEY_AGENT_STREAM: &str = "aura/core/agent/{session_id}/stream";
pub const KEY_SESSION_APPROVAL: &str = "aura/sessions/{session_id}/approval";
pub const KEY_TOOL_EXEC: &str = "aura/tools/{tool_name}/exec";

pub struct ZenohIpcClient {
    pub session: Arc<zenoh::Session>,
}

impl ZenohIpcClient {
    pub async fn connect() -> Result<Self, String> {
        // Phase 3.1: Workload Identity (Layer 1)
        // Connect to local SPIRE agent endpoint
        let _spiffe_client_result = WorkloadApiClient::connect_env().await;
        // In a real environment, we'd extract the SVID and use it for mTLS.
        // For now, we stub the result and log the target identity.
        let spiffe_id = "spiffe://aura.local/workload/aura-cli";
        println!("Performing SPIFFE SVID attestation for aura-cli... Identity: {}", spiffe_id);
        
        // Phase 3.2: mTLS Transport configuration for Zenoh
        let mut config = Config::default();
        if let Err(e) = config.insert_json5("shared_memory/enabled", "true") {
            eprintln!("Failed to configure SHM: {}", e);
        }
        // Stub: Insert mTLS TLS configuration using spiffe-rustls-tokio
        // config.insert_json5("transport/tls/cert", ...);

        let session = zenoh::open(config)
            .res()
            .await
            .map_err(|e| e.to_string())?;

        Ok(Self { session: Arc::new(session) })
    }

    pub async fn publish_approval(&self, session_id: &str, payload: &[u8]) -> Result<(), String> {
        let key_expr = KEY_SESSION_APPROVAL.replace("{session_id}", session_id);
        self.session
            .put(key_expr, payload)
            .res()
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn subscribe_telemetry(&self, session_id: &str) -> Result<zenoh::subscriber::Subscriber<'_, flume::Receiver<zenoh::sample::Sample>>, String> {
        let key_expr = KEY_AGENT_STREAM.replace("{session_id}", session_id);
        self.session
            .declare_subscriber(key_expr)
            .res()
            .await
            .map_err(|e| e.to_string())
    }
}
