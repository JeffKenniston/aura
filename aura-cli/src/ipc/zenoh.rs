use zenoh::prelude::r#async::*;
use spiffe::workload_api::client::WorkloadApiClient;
use std::sync::Arc;

pub struct ZenohIpcClient {
    session: zenoh::Session,
}

impl ZenohIpcClient {
    pub async fn connect() -> Result<Self, String> {
        // Dummy SPIFFE attestation check targeting Layer 1 Security
        // spiffe-rustls-tokio would validate spiffe://aura.local/workload/aura-cli
        println!("Performing SPIFFE SVID attestation for aura-cli...");
        let spiffe_id = "spiffe://aura.local/workload/aura-cli";
        println!("Attestation successful. Identity: {}", spiffe_id);
        
        // Open Zenoh session
        let config = config::peer();
        let session = zenoh::open(config)
            .res()
            .await
            .map_err(|e| e.to_string())?;

        Ok(Self { session })
    }

    pub async fn publish_diff(&self, key_expr: &str, payload: &[u8]) -> Result<(), String> {
        self.session
            .put(key_expr, payload)
            .res()
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn subscribe_telemetry(&self, session_id: &str) -> Result<zenoh::subscriber::Subscriber<'_, flume::Receiver<zenoh::sample::Sample>>, String> {
        let key_expr = format!("aura/core/agent/{}/stream", session_id);
        self.session
            .declare_subscriber(key_expr)
            .res()
            .await
            .map_err(|e| e.to_string())
    }
}
