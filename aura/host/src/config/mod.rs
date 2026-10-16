use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct TransportConfig {
    pub mcp_port: u16,
    pub http3_port: u16,
    pub tcp_fallback_port: u16,
    pub udp_bind_addr: String,
    pub tcp_bind_addr: String,
    pub alt_svc_header: String,
    pub max_idle_timeout_ms: u64,
    pub max_udp_payload_size: u64,
    pub initial_max_data: u64,
    pub initial_max_streams_bidi: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct PricingConfig {
    pub compute_cost_per_ms: f64,
    pub network_cost_per_byte: f64,
    pub storage_cost_per_byte: f64,
    pub billing_webhook_url: String,
    pub webhook_timeout_ms: u64,
    pub hard_cost_budget: f64,
    pub budget_suspension_threshold: f64,
}

impl PricingConfig {
    pub fn calculate_storage_cost_by_days(&self, bytes: u64, days: f64) -> f64 {
        (bytes as f64) * self.storage_cost_per_byte * days
    }

    pub fn calculate_token_cost(&self, input: u64, output: u64) -> f64 {
        (input as f64 + output as f64) * 0.000001
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RuntimeConfig {
    pub worker_threads: usize,
    pub max_blocking_threads: usize,
    pub thread_keep_alive_secs: u64,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            worker_threads: 0,
            max_blocking_threads: 512,
            thread_keep_alive_secs: 0,
        }
    }
}

impl RuntimeConfig {
    pub fn validate(&self) -> Result<(), String> {
        Ok(())
    }
}
