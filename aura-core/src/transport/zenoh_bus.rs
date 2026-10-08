use crate::identity::Svid;
use std::sync::Arc;
use zenoh::prelude::r#async::*;
use zenoh::Session;

pub struct ZenohBus {
    session: Arc<Session>,
    pub prefix: String,
}

impl ZenohBus {
    pub async fn new(svid: &Svid) -> Result<Self, Box<dyn std::error::Error>> {
        let config = zenoh::config::Config::default();
        let session = zenoh::open(config)
            .res_async()
            .await
            .map_err(|e| e.to_string())?;

        // Enforce the SPIFFE SVID topic prefixing boundary
        // E.g., spiffe://aura.local/host becomes spiffe/aura.local/host
        let sanitized_id = svid.id.replace("://", "/").replace("//", "/");
        let prefix = format!("aura/workspace/{}", sanitized_id);

        Ok(Self {
            session: Arc::new(session),
            prefix,
        })
    }

    /// Publishes a payload to a scoped sub-topic.
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

    /// Subscribes to a scoped sub-topic.
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_zenoh_bus_pub_sub() -> Result<(), Box<dyn std::error::Error>> {
        let svid = Svid {
            id: "spiffe://test".to_string(),
            scopes: std::collections::HashSet::new(),
        };
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
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct TaskPayload {
    pub task_id: String,
    pub agent: String,
    pub instruction: String,
    pub substrate: String,
}

#[derive(Debug, Serialize)]
pub struct TaskResult {
    pub task_id: String,
    pub result: String,
}

impl ZenohBus {
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
                    println!("Received task: {:?}", task);

                    let result_str = if task.substrate.eq_ignore_ascii_case("wasm") {
                        // In reality we would call crate::hypervisor::wasm::execute_component
                        // but here we simulate execution completion
                        format!("Executed {} in WASM", task.instruction)
                    } else {
                        // In reality we would call crate::hypervisor::firecracker
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

    pub async fn start_token_monitor(&self) -> Result<(), Box<dyn std::error::Error>> {
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
                "Listening for tasks/metrics/tokens on {}/tasks/metrics/tokens",
                prefix
            );
            while let Ok(sample) = subscriber.recv_async().await {
                let payload_bytes = sample.payload.contiguous();
                let payload_str = String::from_utf8_lossy(&payload_bytes);

                #[derive(Deserialize)]
                struct TokenMetric {
                    agent: String,
                    session_id: String,
                    total_tokens: u32,
                }

                if let Ok(metric) = serde_json::from_str::<TokenMetric>(&payload_str) {
                    println!(
                        "Shadow tracking token usage: {} for agent {}",
                        metric.total_tokens, metric.agent
                    );

                    // Trigger compression at 80% of 1M context (800,000)
                    if metric.total_tokens > 800_000 {
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
}

#[derive(Debug, Deserialize)]
pub struct ToolPayload {
    pub tool_id: String,
    pub tool_type: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct ToolResult {
    pub tool_id: String,
    pub result: String,
}

impl ZenohBus {
    pub async fn start_tools_listener(&self) -> Result<(), Box<dyn std::error::Error>> {
        let subscriber = self
            .session
            .declare_subscriber(format!("{}/tools/execute", self.prefix))
            .res_async()
            .await
            .map_err(|e| e.to_string())?;
        let session_clone = Arc::clone(&self.session);
        let prefix = self.prefix.clone();

        tokio::spawn(async move {
            println!("Listening for tools/execute on {}/tools/execute", prefix);
            while let Ok(sample) = subscriber.recv_async().await {
                let payload_bytes = sample.payload.contiguous();
                let payload_str = String::from_utf8_lossy(&payload_bytes);

                if let Ok(tool_req) = serde_json::from_str::<ToolPayload>(&payload_str) {
                    println!("Received tool execution: {:?}", tool_req);

                    let session_clone = session_clone.clone();
                    let prefix = prefix.clone();
                    tokio::spawn(async move {
                        let tool_id = tool_req.tool_id.clone();
                        let result_str: String = tokio::task::spawn_blocking(
                            move || match tool_req.tool_type.as_str() {
                                "bash" => {
                                    let env = crate::hypervisor::tools::BashEnvironment::new(
                                        &tool_req.tool_id,
                                        "spiffe://aura.local/tool",
                                    );
                                    if let Some(cmd) =
                                        tool_req.payload.get("command").and_then(|c| c.as_str())
                                    {
                                        env.execute(cmd)
                                            .unwrap_or_else(|e| format!("Bash error: {}", e))
                                    } else {
                                        "Missing command for bash tool".to_string()
                                    }
                                }
                                "browser" => {
                                    let env = crate::hypervisor::tools::BrowserEnvironment::new(
                                        &tool_req.tool_id,
                                        "spiffe://aura.local/tool",
                                    );
                                    if let Some(url) =
                                        tool_req.payload.get("url").and_then(|u| u.as_str())
                                    {
                                        env.navigate(url)
                                            .unwrap_or_else(|e| format!("Browser error: {}", e))
                                    } else {
                                        "Missing url for browser tool".to_string()
                                    }
                                }
                                "computer" => {
                                    let env = crate::hypervisor::tools::ComputerEnvironment::new(
                                        &tool_req.tool_id,
                                        "spiffe://aura.local/tool",
                                    );
                                    let x = tool_req
                                        .payload
                                        .get("x")
                                        .and_then(|v| v.as_u64())
                                        .unwrap_or(0)
                                        as u32;
                                    let y = tool_req
                                        .payload
                                        .get("y")
                                        .and_then(|v| v.as_u64())
                                        .unwrap_or(0)
                                        as u32;
                                    env.click(x, y)
                                        .unwrap_or_else(|e| format!("Computer error: {}", e))
                                }
                                "filesystem" => {
                                    let action = tool_req
                                        .payload
                                        .get("action")
                                        .and_then(|a| a.as_str())
                                        .unwrap_or("");
                                    let path = tool_req
                                        .payload
                                        .get("path")
                                        .and_then(|p| p.as_str())
                                        .unwrap_or("");

                                    match action {
                                        "read" => {
                                            crate::hypervisor::wasm_tools::WasmTools::fs_read(path)
                                                .unwrap_or_else(|e| {
                                                    format!("FileSystem read error: {}", e)
                                                })
                                        }
                                        "write" => {
                                            let content = tool_req
                                                .payload
                                                .get("content")
                                                .and_then(|c| c.as_str())
                                                .unwrap_or("");
                                            crate::hypervisor::wasm_tools::WasmTools::fs_write(
                                                path, content,
                                            )
                                            .map(|_| "File written successfully".to_string())
                                            .unwrap_or_else(|e| {
                                                format!("FileSystem write error: {}", e)
                                            })
                                        }
                                        _ => format!("Unknown filesystem action: {}", action),
                                    }
                                }
                                "ast-blast-radius" => {
                                    let target = tool_req
                                        .payload
                                        .get("target_symbol_id")
                                        .and_then(|t| t.as_str())
                                        .unwrap_or("");
                                    crate::hypervisor::wasm_tools::WasmTools::analyze_blast_radius(
                                        target,
                                    )
                                    .map(|count| count.to_string())
                                    .unwrap_or_else(|e| format!("AST Blast Radius error: {}", e))
                                }
                                "ephemeral-tunnel" => {
                                    let agent_name = tool_req
                                        .payload
                                        .get("agent_name")
                                        .and_then(|a| a.as_str())
                                        .unwrap_or("unknown");
                                    crate::hypervisor::wasm_tools::WasmTools::mint_ephemeral_svid(
                                        agent_name,
                                    )
                                    .unwrap_or_else(|e| format!("Ephemeral Tunnel error: {}", e))
                                }
                                _ => format!("Unknown tool type: {}", tool_req.tool_type),
                            },
                        )
                        .await
                        .unwrap_or_else(|e| format!("Task error: {}", e));

                        let res = ToolResult {
                            tool_id,
                            result: result_str,
                        };

                        if let Ok(json_res) = serde_json::to_string(&res) {
                            let _ = session_clone
                                .put(format!("{}/tools/result", prefix), json_res)
                                .res_async()
                                .await;
                        }
                    });
                }
            }
        });

        Ok(())
    }
}
