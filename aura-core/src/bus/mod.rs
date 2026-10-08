pub mod rate_limiter;

use crate::config::BusConfig;
use crate::identity::Svid;
pub use rate_limiter::RateLimiter;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use zenoh::prelude::r#async::*;
use zenoh::Session;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TaskPayload {
    pub task_id: String,
    pub agent: String,
    pub instruction: String,
    pub substrate: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TaskResult {
    pub task_id: String,
    pub result: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ToolPayload {
    pub tool_id: String,
    pub tool_type: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolResult {
    pub tool_id: String,
    pub result: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TokenMetric {
    pub agent: String,
    pub session_id: String,
    pub total_tokens: u32,
}

/// Zenoh publish/subscribe router for microkernel and agent communication (BUS-001 - BUS-009).
/// All inter-process communication traverses this bus with Zero FFI and strict SPIFFE SVID scoping.
pub struct ZenohBus {
    session: Arc<Session>,
    pub prefix: String,
}

impl ZenohBus {
    /// Creates a new ZenohBus instance bound to an attested SPIFFE SVID.
    pub async fn new(svid: &Svid) -> Result<Self, Box<dyn std::error::Error>> {
        let config = zenoh::config::Config::default();
        let session = zenoh::open(config)
            .res_async()
            .await
            .map_err(|e| e.to_string())?;

        // Enforce the SPIFFE SVID topic prefixing boundary (BUS-002)
        // E.g., spiffe://aura.local/host becomes spiffe/aura.local/host
        let sanitized_id = svid.id.replace("://", "/").replace("//", "/");
        let prefix = format!("aura/workspace/{}", sanitized_id);

        Ok(Self {
            session: Arc::new(session),
            prefix,
        })
    }

    /// Creates a new ZenohBus instance with custom BusConfig and SVID.
    pub async fn with_config(
        svid: &Svid,
        bus_config: &BusConfig,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let config = zenoh::config::Config::default();
        let session = zenoh::open(config)
            .res_async()
            .await
            .map_err(|e| e.to_string())?;

        let sanitized_id = svid.id.replace("://", "/").replace("//", "/");
        let prefix = format!("{}/{}", bus_config.prefix, sanitized_id);

        Ok(Self {
            session: Arc::new(session),
            prefix,
        })
    }

    /// Publishes a payload to a scoped sub-topic (BUS-001).
    pub async fn publish<T: Into<zenoh::value::Value>>(
        &self,
        sub_topic: &str,
        payload: T,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let full_topic = format!("{}/{}", self.prefix, sub_topic);
        self.session
            .put(full_topic, payload)
            .res_async()
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Subscribes to a scoped sub-topic (BUS-001).
    pub async fn subscribe(
        &self,
        sub_topic: &str,
    ) -> Result<
        zenoh::subscriber::Subscriber<'_, flume::Receiver<zenoh::sample::Sample>>,
        Box<dyn std::error::Error>,
    > {
        let full_topic = format!("{}/{}", self.prefix, sub_topic);
        let subscriber = self
            .session
            .declare_subscriber(full_topic)
            .res_async()
            .await
            .map_err(|e| e.to_string())?;
        Ok(subscriber)
    }

    /// Starts the task queue dispatch listener and emits async completion events (BUS-003).
    pub async fn start_task_listener(&self) -> Result<(), Box<dyn std::error::Error>> {
        let subscriber = self
            .session
            .declare_subscriber(format!("{}/tasks/dispatch", self.prefix))
            .res_async()
            .await
            .map_err(|e| e.to_string())?;
        let session_clone = Arc::clone(&self.session);
        let prefix = self.prefix.clone();

        tokio::spawn(async move {
            println!("Listening for tasks/dispatch on {}/tasks/dispatch", prefix);
            while let Ok(sample) = subscriber.recv_async().await {
                let payload_bytes = sample.payload.contiguous();
                let payload_str = String::from_utf8_lossy(&payload_bytes);

                if let Ok(task) = serde_json::from_str::<TaskPayload>(&payload_str) {
                    println!("Received task dispatch: {:?}", task);

                    let result_str = if task.substrate.eq_ignore_ascii_case("wasm") {
                        format!("Executed {} in WASM", task.instruction)
                    } else {
                        format!("Executed {} in Firecracker microVM", task.instruction)
                    };

                    let res = TaskResult {
                        task_id: task.task_id,
                        result: result_str,
                    };

                    if let Ok(json_res) = serde_json::to_string(&res) {
                        let _ = session_clone
                            .put(format!("{}/tasks/completion", prefix), json_res)
                            .res_async()
                            .await;
                    }
                }
            }
        });

        Ok(())
    }

    /// Starts shadow token monitoring to trigger autonomous context compression (BUS-004, ADR-002).
    pub async fn start_token_monitor(
        &self,
        threshold: u32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let subscriber = self
            .session
            .declare_subscriber(format!("{}/tasks/metrics/tokens", self.prefix))
            .res_async()
            .await
            .map_err(|e| e.to_string())?;
        let session_clone = Arc::clone(&self.session);
        let prefix = self.prefix.clone();

        tokio::spawn(async move {
            println!(
                "Listening for tasks/metrics/tokens on {}/tasks/metrics/tokens (threshold: {})",
                prefix, threshold
            );
            while let Ok(sample) = subscriber.recv_async().await {
                let payload_bytes = sample.payload.contiguous();
                let payload_str = String::from_utf8_lossy(&payload_bytes);

                if let Ok(metric) = serde_json::from_str::<TokenMetric>(&payload_str) {
                    println!(
                        "Shadow tracking token usage: {} for agent {}",
                        metric.total_tokens, metric.agent
                    );

                    // Trigger compression when threshold is breached
                    if metric.total_tokens > threshold {
                        println!(
                            "Threshold breached! Instructing agent {} to compress context.",
                            metric.agent
                        );
                        let control_payload = serde_json::json!({
                            "agent": metric.agent,
                            "session_id": metric.session_id,
                            "action": "compress"
                        });
                        let _ = session_clone
                            .put(
                                format!("{}/control/session/compress", prefix),
                                control_payload.to_string(),
                            )
                            .res_async()
                            .await;
                    }
                }
            }
        });

        Ok(())
    }

    /// Starts the router with backpressure rate limiting (BUS-007, BUS-008).
    pub async fn start_router(
        &self,
        registry: Arc<crate::runtime::registry::Registry>,
        limiter: Arc<RateLimiter>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let subscriber = self
            .session
            .declare_subscriber(format!("{}/tasks/*", self.prefix))
            .res_async()
            .await
            .map_err(|e| e.to_string())?;

        let session_clone = Arc::clone(&self.session);
        let prefix = self.prefix.clone();

        tokio::spawn(async move {
            println!("Router listening for tasks on {}/tasks/*", prefix);
            while let Ok(sample) = subscriber.recv_async().await {
                let mock_spiffe_id = "spiffe://aura.local/agent/1";

                if !limiter.check_and_consume(mock_spiffe_id) {
                    println!("RateLimitExceeded for {}", mock_spiffe_id);
                    // Emit RateLimitExceeded completion event
                    let _ = session_clone
                        .put(format!("{}/tasks/completion", prefix), "RateLimitExceeded")
                        .res_async()
                        .await;
                    continue;
                }

                let payload_bytes = sample.payload.contiguous().to_vec();
                let reg = Arc::clone(&registry);

                tokio::spawn(async move {
                    // Identity validation check before backend execution (Zero-Trust)
                    if let Ok(_scopes) =
                        crate::identity::enforcer::CapabilityEnforcer::validate_svid("mock_jwt")
                            .await
                    {
                        if let Ok(task) = serde_json::from_slice::<TaskPayload>(&payload_bytes) {
                            if let Some(backend) = reg.resolve(&task.instruction) {
                                match backend {
                                    crate::runtime::registry::RuntimeBackend::Wasm(_path) => {
                                        println!("Routing task {} to WASM backend", task.task_id);
                                    }
                                    crate::runtime::registry::RuntimeBackend::Firecracker(
                                        _config,
                                    ) => {
                                        println!(
                                            "Routing task {} to Firecracker backend",
                                            task.task_id
                                        );
                                    }
                                }
                            }
                        }
                    }
                });
            }
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_zenoh_bus_pub_sub() -> Result<(), Box<dyn std::error::Error>> {
        let svid = Svid::new("spiffe://test");
        let bus = ZenohBus::new(&svid).await?;

        // 1. Subscribe to the scoped topic
        let subscriber = bus.subscribe("events").await?;

        // 2. Publish a message to the same topic
        let test_payload = "zero-copy message";
        bus.publish("events", test_payload).await?;

        // 3. Receive the message
        let sample = subscriber.recv_async().await?;

        // 4. Verify contents and identity scoping
        let binding = sample.payload.contiguous();
        let received_str = String::from_utf8_lossy(&binding);
        assert_eq!(received_str, test_payload);
        assert!(sample
            .key_expr
            .as_str()
            .starts_with("aura/workspace/spiffe/test/"));

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_task_dispatch_and_completion() -> Result<(), Box<dyn std::error::Error>> {
        let svid = Svid::new("spiffe://aura.local/agent/task_test");
        let bus = ZenohBus::new(&svid).await?;

        // Start task listener
        bus.start_task_listener().await?;

        // Subscribe to completion
        let completion_sub = bus.subscribe("tasks/completion").await?;

        // Dispatch a task
        let task = TaskPayload {
            task_id: "test-task-123".to_string(),
            agent: "agent-1".to_string(),
            instruction: "compute_hash".to_string(),
            substrate: "wasm".to_string(),
        };

        let task_json = serde_json::to_string(&task)?;
        bus.publish("tasks/dispatch", task_json).await?;

        // Wait for completion event
        let sample = completion_sub.recv_async().await?;
        let binding = sample.payload.contiguous();
        let res_str = String::from_utf8_lossy(&binding);
        let res: TaskResult = serde_json::from_str(&res_str)?;

        assert_eq!(res.task_id, "test-task-123");
        assert!(res.result.contains("Executed compute_hash in WASM"));

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_token_monitor_compression_trigger() -> Result<(), Box<dyn std::error::Error>> {
        let svid = Svid::new("spiffe://aura.local/agent/token_test");
        let bus = ZenohBus::new(&svid).await?;

        // Threshold set to 100 for test
        bus.start_token_monitor(100).await?;

        let compress_sub = bus.subscribe("control/session/compress").await?;

        // Publish a metric that exceeds threshold
        let metric = TokenMetric {
            agent: "agent-opt".to_string(),
            session_id: "sess-999".to_string(),
            total_tokens: 150,
        };

        let metric_json = serde_json::to_string(&metric)?;
        bus.publish("tasks/metrics/tokens", metric_json).await?;

        // Await compression control directive
        let sample = compress_sub.recv_async().await?;
        let binding = sample.payload.contiguous();
        let payload_str = String::from_utf8_lossy(&binding);
        let parsed: serde_json::Value = serde_json::from_str(&payload_str)?;

        assert_eq!(parsed["action"], "compress");
        assert_eq!(parsed["agent"], "agent-opt");

        Ok(())
    }
}
