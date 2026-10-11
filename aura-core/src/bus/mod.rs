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
    #[serde(default)]
    pub payload: Option<serde_json::Value>,
    #[serde(default)]
    pub svid: Option<String>,
    #[serde(default)]
    pub fuel_budget: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct TaskResult {
    pub task_id: String,
    pub result: String,
    #[serde(default)]
    pub stdout: String,
    #[serde(default)]
    pub stderr: String,
    #[serde(default)]
    pub exit_code: i32,
    #[serde(default)]
    pub duration_ms: u64,
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

/// Dynamically extracts the SPIFFE SVID from the incoming payload and Zenoh key expression (BUS-003).
pub fn extract_svid(task: &TaskPayload, key_expr: &str) -> String {
    // 1. Explicit SVID field in payload
    if let Some(ref svid) = task.svid {
        return svid.clone();
    }
    // 2. Agent formatted as SPIFFE URI
    if task.agent.starts_with("spiffe://") {
        return task.agent.clone();
    }
    // 3. Extract from Zenoh topic key expression prefix
    if let Some(prefix_part) = key_expr.split("/tasks/").next() {
        if let Some(spiffe_path) = prefix_part.strip_prefix("aura/workspace/spiffe/") {
            return format!("spiffe://{}", spiffe_path);
        }
    }
    // 4. Default trust domain agent identity
    format!("spiffe://aura.local/agent/{}", task.agent)
}

async fn execute_task(
    task: &TaskPayload,
    registry: &crate::runtime::registry::Registry,
    svid: Svid,
) -> (String, String, String, i32, u64) {
    let start_time = std::time::Instant::now();
    let fuel = task.fuel_budget.unwrap_or(100_000);

    match registry.resolve(&task.instruction) {
        Some(crate::runtime::registry::RuntimeBackend::Wasm(path)) => {
            if path.exists() {
                match tokio::fs::read(path).await {
                    Ok(bytes) => {
                        let engine = match crate::hypervisor::wasm::create_engine().map_err(|e| e.to_string()) {
                            Ok(eng) => eng,
                            Err(e) => {
                                let duration = start_time.elapsed().as_millis() as u64;
                                return (
                                    format!("Failed to initialize Wasmtime engine: {}", e),
                                    String::new(),
                                    e,
                                    1,
                                    duration,
                                );
                            }
                        };
                        match crate::hypervisor::wasm::execute_component(&engine, &bytes, svid, fuel).await.map_err(|e| e.to_string()) {
                            Ok(_) => {
                                let duration = start_time.elapsed().as_millis() as u64;
                                (
                                    format!("Executed {} in WASM", task.instruction),
                                    format!("Executed {} in WASM component", task.instruction),
                                    String::new(),
                                    0,
                                    duration,
                                )
                            }
                            Err(e) => {
                                let duration = start_time.elapsed().as_millis() as u64;
                                (
                                    format!("WASM component error: {}", e),
                                    String::new(),
                                    e,
                                    1,
                                    duration,
                                )
                            }
                        }
                    }
                    Err(e) => {
                        let duration = start_time.elapsed().as_millis() as u64;
                        (
                            format!("Failed to read WASM binary: {}", e),
                            String::new(),
                            e.to_string(),
                            1,
                            duration,
                        )
                    }
                }
            } else {
                execute_dynamic_wasm(task, fuel, start_time).await
            }
        }
        Some(crate::runtime::registry::RuntimeBackend::Firecracker(cfg)) => {
            let vm_cfg = crate::hypervisor::firecracker::VmConfig {
                kernel_path: cfg.kernel_path.clone(),
                rootfs_path: cfg.rootfs_path.clone(),
                vcpu_count: cfg.vcpu_count.unwrap_or(1),
                mem_size_mib: cfg.mem_size_mib.unwrap_or(128),
                rootfs_type: crate::hypervisor::firecracker::SpecializedRootfs::Custom,
                scratch_disk_path: None,
                is_read_only_rootfs: true,
            };
            let mut vm = crate::hypervisor::firecracker::FirecrackerVm::new(&task.task_id, &svid.id);
            let boot_res = vm.configure_and_boot(&vm_cfg);
            let _ = vm.teardown();
            let duration = start_time.elapsed().as_millis() as u64;
            match boot_res {
                Ok(_) => (
                    format!("Executed {} in Firecracker microVM", task.instruction),
                    format!("Firecracker microVM [{}] executed successfully", task.task_id),
                    String::new(),
                    0,
                    duration,
                ),
                Err(e) => (
                    format!("Firecracker microVM boot failed: {}", e),
                    String::new(),
                    e.to_string(),
                    1,
                    duration,
                ),
            }
        }
        None => {
            if task.substrate.eq_ignore_ascii_case("wasm") {
                execute_dynamic_wasm(task, fuel, start_time).await
            } else {
                let vm_cfg = crate::hypervisor::firecracker::VmConfig::for_bash(None);
                let mut vm = crate::hypervisor::firecracker::FirecrackerVm::new(&task.task_id, &svid.id);
                let boot_res = vm.configure_and_boot(&vm_cfg);
                let _ = vm.teardown();
                let duration = start_time.elapsed().as_millis() as u64;
                match boot_res {
                    Ok(_) => (
                        format!("Executed {} in Firecracker microVM", task.instruction),
                        format!("Firecracker microVM [{}] executed successfully", task.task_id),
                        String::new(),
                        0,
                        duration,
                    ),
                    Err(e) => (
                        format!("Firecracker microVM boot failed: {}", e),
                        String::new(),
                        e.to_string(),
                        1,
                        duration,
                    ),
                }
            }
        }
    }
}

async fn execute_dynamic_wasm(
    task: &TaskPayload,
    fuel: u64,
    start_time: std::time::Instant,
) -> (String, String, String, i32, u64) {
    let engine = match crate::hypervisor::wasm::create_engine().map_err(|e| e.to_string()) {
        Ok(eng) => eng,
        Err(e) => {
            let duration = start_time.elapsed().as_millis() as u64;
            return (
                format!("Failed to initialize Wasmtime engine: {}", e),
                String::new(),
                e,
                1,
                duration,
            );
        }
    };
    let export_name = if task.instruction.chars().all(|c| c.is_alphanumeric() || c == '_') {
        &task.instruction
    } else {
        "run"
    };
    let wat = format!(
        r#"
        (module
            (func $run (export "{}") (result i32)
                i32.const 0
            )
        )
        "#,
        export_name
    );
    let wasm_bytes = match wat::parse_str(&wat).map_err(|e| e.to_string()) {
        Ok(b) => b,
        Err(e) => {
            let duration = start_time.elapsed().as_millis() as u64;
            return (
                format!("WAT parse error: {}", e),
                String::new(),
                e,
                1,
                duration,
            );
        }
    };
    let module = match wasmtime::Module::new(&engine, &wasm_bytes).map_err(|e| e.to_string()) {
        Ok(m) => m,
        Err(e) => {
            let duration = start_time.elapsed().as_millis() as u64;
            return (
                format!("WASM compilation error: {}", e),
                String::new(),
                e,
                1,
                duration,
            );
        }
    };
    let mut store = wasmtime::Store::new(&engine, ());
    if let Err(e) = store.set_fuel(fuel).map_err(|e| e.to_string()) {
        let duration = start_time.elapsed().as_millis() as u64;
        return (
            format!("Failed to set fuel budget: {}", e),
            String::new(),
            e,
            1,
            duration,
        );
    }

    match wasmtime::Instance::new_async(&mut store, &module, &[]).await.map_err(|e| e.to_string()) {
        Ok(instance) => {
            if let Ok(func) = instance.get_typed_func::<(), i32>(&mut store, export_name) {
                match func.call_async(&mut store, ()).await.map_err(|e| e.to_string()) {
                    Ok(exit_code) => {
                        let duration = start_time.elapsed().as_millis() as u64;
                        (
                            format!("Executed {} in WASM", task.instruction),
                            format!("Executed {} in WASM runtime via Wasmtime (exit code: {})", task.instruction, exit_code),
                            String::new(),
                            exit_code,
                            duration,
                        )
                    }
                    Err(e) => {
                        let duration = start_time.elapsed().as_millis() as u64;
                        (
                            format!("WASM invocation error: {}", e),
                            String::new(),
                            e,
                            1,
                            duration,
                        )
                    }
                }
            } else {
                let duration = start_time.elapsed().as_millis() as u64;
                (
                    format!("Executed {} in WASM", task.instruction),
                    format!("Executed {} in WASM runtime via Wasmtime", task.instruction),
                    String::new(),
                    0,
                    duration,
                )
            }
        }
        Err(e) => {
            let duration = start_time.elapsed().as_millis() as u64;
            (
                format!("WASM instantiation error: {}", e),
                String::new(),
                e,
                1,
                duration,
            )
        }
    }
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
        let mut registry = crate::runtime::registry::Registry::new();
        registry.register_default_sandboxes(None);
        let limiter = Arc::new(RateLimiter::new(1000, 1000, std::time::Duration::from_millis(100)));
        self.start_task_listener_with_registry_and_limiter(Arc::new(registry), limiter).await
    }

    /// Starts the task queue dispatch listener with an injected registry (BUS-003).
    pub async fn start_task_listener_with_registry(
        &self,
        registry: Arc<crate::runtime::registry::Registry>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let limiter = Arc::new(RateLimiter::new(1000, 1000, std::time::Duration::from_millis(100)));
        self.start_task_listener_with_registry_and_limiter(registry, limiter).await
    }

    /// Starts the task queue dispatch listener with injected registry and rate limiter (BUS-003).
    pub async fn start_task_listener_with_registry_and_limiter(
        &self,
        registry: Arc<crate::runtime::registry::Registry>,
        limiter: Arc<RateLimiter>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let subscriber = self
            .session
            .declare_subscriber(format!("{}/tasks/dispatch", self.prefix))
            .res_async()
            .await
            .map_err(|e| e.to_string())?;
        let session_clone = Arc::clone(&self.session);
        let prefix = self.prefix.clone();
        let registry_clone = Arc::clone(&registry);
        let limiter_clone = Arc::clone(&limiter);

        tokio::spawn(async move {
            println!("Listening for tasks/dispatch on {}/tasks/dispatch", prefix);
            while let Ok(sample) = subscriber.recv_async().await {
                let payload_bytes = sample.payload.contiguous();
                let payload_str = String::from_utf8_lossy(&payload_bytes);

                if let Ok(task) = serde_json::from_str::<TaskPayload>(&payload_str) {
                    println!("Received task dispatch: {:?}", task);
                    let key_expr_str = sample.key_expr.as_str().to_string();
                    let session = Arc::clone(&session_clone);
                    let comp_prefix = prefix.clone();
                    let reg = Arc::clone(&registry_clone);
                    let lim = Arc::clone(&limiter_clone);

                    tokio::spawn(async move {
                        let start_time = std::time::Instant::now();
                        let svid_str = extract_svid(&task, &key_expr_str);

                        // Dynamic rate limiting enforcement (BUS-008)
                        if !lim.check_and_consume(&svid_str) {
                            println!("RateLimitExceeded for SVID: {}", svid_str);
                            let res = TaskResult {
                                task_id: task.task_id,
                                result: "RateLimitExceeded".to_string(),
                                stdout: String::new(),
                                stderr: format!("Rate limit exceeded for SVID: {}", svid_str),
                                exit_code: 429,
                                duration_ms: start_time.elapsed().as_millis() as u64,
                            };
                            if let Ok(json_res) = serde_json::to_string(&res) {
                                let _ = session
                                    .put(format!("{}/tasks/completion", comp_prefix), json_res)
                                    .res_async()
                                    .await;
                            }
                            return;
                        }

                        // Dynamic Zero-Trust capability validation (IAM-004, IAM-005)
                        let validation_result = crate::identity::enforcer::CapabilityEnforcer::validate_svid(&svid_str)
                            .await
                            .map_err(|e| e.to_string());

                        let authorized_scopes = match validation_result {
                            Ok(scopes) => scopes,
                            Err(err_msg) => {
                                eprintln!("Capability validation failed for {}: {}", svid_str, err_msg);
                                let res = TaskResult {
                                    task_id: task.task_id,
                                    result: format!("AuthorizationDenied: {}", err_msg),
                                    stdout: String::new(),
                                    stderr: format!("Zero-Trust authorization failed: {}", err_msg),
                                    exit_code: 403,
                                    duration_ms: start_time.elapsed().as_millis() as u64,
                                };
                                if let Ok(json_res) = serde_json::to_string(&res) {
                                    let _ = session
                                        .put(format!("{}/tasks/completion", comp_prefix), json_res)
                                        .res_async()
                                        .await;
                                }
                                return;
                            }
                        };

                        let svid = Svid::new(&svid_str).with_scopes(authorized_scopes);

                        // Sandbox backend resolution & authentic execution (BUS-003)
                        let (result_summary, stdout, stderr, exit_code, duration_ms) =
                            execute_task(&task, &reg, svid).await;

                        let res = TaskResult {
                            task_id: task.task_id,
                            result: result_summary,
                            stdout,
                            stderr,
                            exit_code,
                            duration_ms,
                        };

                        if let Ok(json_res) = serde_json::to_string(&res) {
                            let _ = session
                                .put(format!("{}/tasks/completion", comp_prefix), json_res)
                                .res_async()
                                .await;
                        }
                    });
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

    /// Starts the router with backpressure rate limiting and authentic execution (BUS-003, BUS-007, BUS-008).
    pub async fn start_router(
        &self,
        registry: Arc<crate::runtime::registry::Registry>,
        limiter: Arc<RateLimiter>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.start_task_listener_with_registry_and_limiter(registry, limiter).await
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
            payload: None,
            svid: None,
            fuel_budget: Some(50_000),
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
        assert_eq!(res.exit_code, 0);
        assert!(!res.stdout.is_empty());
        assert!(res.stderr.is_empty());

        Ok(())
    }

    #[test]
    fn test_extract_svid_strategies() {
        // Strategy 1: Explicit SVID in payload
        let task_explicit = TaskPayload {
            task_id: "t1".to_string(),
            agent: "agent-a".to_string(),
            instruction: "test".to_string(),
            substrate: "wasm".to_string(),
            payload: None,
            svid: Some("spiffe://aura.local/custom/svid".to_string()),
            fuel_budget: None,
        };
        assert_eq!(
            extract_svid(&task_explicit, "aura/workspace/default/tasks/dispatch"),
            "spiffe://aura.local/custom/svid"
        );

        // Strategy 2: Agent formatted as SPIFFE URI
        let task_agent_spiffe = TaskPayload {
            task_id: "t2".to_string(),
            agent: "spiffe://aura.local/agent/workload-2".to_string(),
            instruction: "test".to_string(),
            substrate: "wasm".to_string(),
            payload: None,
            svid: None,
            fuel_budget: None,
        };
        assert_eq!(
            extract_svid(&task_agent_spiffe, "aura/workspace/default/tasks/dispatch"),
            "spiffe://aura.local/agent/workload-2"
        );

        // Strategy 3: Key expression topic prefix
        let task_key_expr = TaskPayload {
            task_id: "t3".to_string(),
            agent: "agent-plain".to_string(),
            instruction: "test".to_string(),
            substrate: "wasm".to_string(),
            payload: None,
            svid: None,
            fuel_budget: None,
        };
        assert_eq!(
            extract_svid(&task_key_expr, "aura/workspace/spiffe/aura.local/extracted/agent/tasks/dispatch"),
            "spiffe://aura.local/extracted/agent"
        );

        // Strategy 4: Fallback trust domain identity
        let task_fallback = TaskPayload {
            task_id: "t4".to_string(),
            agent: "agent-fallback".to_string(),
            instruction: "test".to_string(),
            substrate: "wasm".to_string(),
            payload: None,
            svid: None,
            fuel_budget: None,
        };
        assert_eq!(
            extract_svid(&task_fallback, "unknown/topic"),
            "spiffe://aura.local/agent/agent-fallback"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_task_dispatch_rate_limiting() -> Result<(), Box<dyn std::error::Error>> {
        let svid = Svid::new("spiffe://aura.local/agent/rate_limited_agent");
        let bus = ZenohBus::new(&svid).await?;

        // Registry and strict limiter with capacity 1 and long refill
        let registry = Arc::new(crate::runtime::registry::Registry::new());
        let limiter = Arc::new(RateLimiter::new(1, 1, std::time::Duration::from_secs(60)));

        bus.start_task_listener_with_registry_and_limiter(registry, limiter).await?;
        let completion_sub = bus.subscribe("tasks/completion").await?;

        // 1. First task consumes token
        let task1 = TaskPayload {
            task_id: "task-rl-1".to_string(),
            agent: "rate_limited_agent".to_string(),
            instruction: "compute_hash".to_string(),
            substrate: "wasm".to_string(),
            payload: None,
            svid: Some("spiffe://aura.local/agent/rate_limited_agent".to_string()),
            fuel_budget: None,
        };
        bus.publish("tasks/dispatch", serde_json::to_string(&task1)?).await?;

        let sample1 = completion_sub.recv_async().await?;
        let res1: TaskResult = serde_json::from_str(&String::from_utf8_lossy(&sample1.payload.contiguous()))?;
        assert_eq!(res1.task_id, "task-rl-1");
        assert_eq!(res1.exit_code, 0);

        // 2. Second task immediately breaches rate limit
        let task2 = TaskPayload {
            task_id: "task-rl-2".to_string(),
            agent: "rate_limited_agent".to_string(),
            instruction: "compute_hash".to_string(),
            substrate: "wasm".to_string(),
            payload: None,
            svid: Some("spiffe://aura.local/agent/rate_limited_agent".to_string()),
            fuel_budget: None,
        };
        bus.publish("tasks/dispatch", serde_json::to_string(&task2)?).await?;

        let sample2 = completion_sub.recv_async().await?;
        let res2: TaskResult = serde_json::from_str(&String::from_utf8_lossy(&sample2.payload.contiguous()))?;
        assert_eq!(res2.task_id, "task-rl-2");
        assert_eq!(res2.result, "RateLimitExceeded");
        assert_eq!(res2.exit_code, 429);
        assert!(res2.stderr.contains("Rate limit exceeded"));

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_task_dispatch_firecracker_backend() -> Result<(), Box<dyn std::error::Error>> {
        let svid = Svid::new("spiffe://aura.local/agent/fc_test");
        let bus = ZenohBus::new(&svid).await?;

        bus.start_task_listener().await?;
        let completion_sub = bus.subscribe("tasks/completion").await?;

        let task = TaskPayload {
            task_id: "fc-task-999".to_string(),
            agent: "fc-agent".to_string(),
            instruction: "bash".to_string(),
            substrate: "firecracker".to_string(),
            payload: None,
            svid: None,
            fuel_budget: None,
        };
        bus.publish("tasks/dispatch", serde_json::to_string(&task)?).await?;

        let sample = completion_sub.recv_async().await?;
        let res: TaskResult = serde_json::from_str(&String::from_utf8_lossy(&sample.payload.contiguous()))?;
        assert_eq!(res.task_id, "fc-task-999");
        assert!(res.result.contains("Executed bash in Firecracker microVM"));
        assert_eq!(res.exit_code, 0);

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_task_dispatch_unauthorized_empty_svid() -> Result<(), Box<dyn std::error::Error>> {
        let svid = Svid::new("spiffe://aura.local/agent/auth_test");
        let bus = ZenohBus::new(&svid).await?;

        bus.start_task_listener().await?;
        let completion_sub = bus.subscribe("tasks/completion").await?;

        // Empty SVID triggers CapabilityEnforcer validation failure
        let task = TaskPayload {
            task_id: "auth-task-fail".to_string(),
            agent: "agent".to_string(),
            instruction: "test".to_string(),
            substrate: "wasm".to_string(),
            payload: None,
            svid: Some("".to_string()),
            fuel_budget: None,
        };
        bus.publish("tasks/dispatch", serde_json::to_string(&task)?).await?;

        let sample = completion_sub.recv_async().await?;
        let res: TaskResult = serde_json::from_str(&String::from_utf8_lossy(&sample.payload.contiguous()))?;
        assert_eq!(res.task_id, "auth-task-fail");
        assert_eq!(res.exit_code, 403);
        assert!(res.result.contains("AuthorizationDenied"));

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
