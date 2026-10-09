use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::Notify;
use zenoh::prelude::r#async::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TenantQueueStatus {
    Active,
    Suspended,
    Terminated,
}

#[derive(Debug)]
pub enum BillingError {
    TenantTerminated(String),
    BillingRejected { status: u16, message: String },
    Timeout(String),
    Network(String),
}

impl fmt::Display for BillingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BillingError::TenantTerminated(t) => {
                write!(f, "Tenant '{}' was terminated due to billing failure", t)
            }
            BillingError::BillingRejected { status, message } => {
                write!(
                    f,
                    "Billing system rejected request with status {}: {}",
                    status, message
                )
            }
            BillingError::Timeout(msg) => write!(f, "Billing webhook timed out: {}", msg),
            BillingError::Network(msg) => write!(f, "Billing webhook network error: {}", msg),
        }
    }
}

impl std::error::Error for BillingError {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingWebhookPayload {
    pub event: String,
    pub tenant_id: String,
    pub current_spend: f64,
    pub hard_budget: f64,
    pub threshold_percentage: f64,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BillingOutcome {
    Resumed,
    Terminated,
}

struct TenantState {
    status: TenantQueueStatus,
    notify: Arc<Notify>,
}

/// Manages Zenoh task queue suspension and synchronous billing webhooks (RTE-009).
pub struct QueueSuspensionManager {
    tenants: RwLock<HashMap<String, Arc<Mutex<TenantState>>>>,
    http_client: reqwest::Client,
    default_timeout: Duration,
}

impl Default for QueueSuspensionManager {
    fn default() -> Self {
        Self::new(Duration::from_millis(5000))
    }
}

impl QueueSuspensionManager {
    pub fn new(default_timeout: Duration) -> Self {
        Self {
            tenants: RwLock::new(HashMap::new()),
            http_client: reqwest::Client::builder()
                .timeout(default_timeout)
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            default_timeout,
        }
    }

    fn get_or_create_state(&self, tenant_id: &str) -> Arc<Mutex<TenantState>> {
        let mut map = self.tenants.write().unwrap();
        map.entry(tenant_id.to_string())
            .or_insert_with(|| {
                Arc::new(Mutex::new(TenantState {
                    status: TenantQueueStatus::Active,
                    notify: Arc::new(Notify::new()),
                }))
            })
            .clone()
    }

    /// Suspends message processing for a tenant's Zenoh queue.
    pub fn suspend_tenant(&self, tenant_id: &str) {
        let state_arc = self.get_or_create_state(tenant_id);
        let mut state = state_arc.lock().unwrap();
        state.status = TenantQueueStatus::Suspended;
        println!(
            "[QueueSuspensionManager] Tenant '{}' task queue suspended.",
            tenant_id
        );
    }

    /// Resumes message processing for a tenant's Zenoh queue upon 200 OK.
    pub fn resume_tenant(&self, tenant_id: &str) {
        let state_arc = self.get_or_create_state(tenant_id);
        let notify = {
            let mut state = state_arc.lock().unwrap();
            state.status = TenantQueueStatus::Active;
            state.notify.clone()
        };
        notify.notify_waiters();
        println!(
            "[QueueSuspensionManager] Tenant '{}' task queue resumed after 200 OK.",
            tenant_id
        );
    }

    /// Gracefully terminates a tenant if billing webhook fails or times out.
    pub fn terminate_tenant(&self, tenant_id: &str) {
        let state_arc = self.get_or_create_state(tenant_id);
        let notify = {
            let mut state = state_arc.lock().unwrap();
            state.status = TenantQueueStatus::Terminated;
            state.notify.clone()
        };
        notify.notify_waiters();
        println!(
            "[QueueSuspensionManager] Tenant '{}' terminated due to billing failure.",
            tenant_id
        );
    }

    /// Returns current queue status for a tenant.
    pub fn get_status(&self, tenant_id: &str) -> TenantQueueStatus {
        let state_arc = self.get_or_create_state(tenant_id);
        let state = state_arc.lock().unwrap();
        state.status
    }

    /// Returns true if a tenant is currently suspended.
    pub fn is_suspended(&self, tenant_id: &str) -> bool {
        self.get_status(tenant_id) == TenantQueueStatus::Suspended
    }

    /// Waits asynchronously if the tenant queue is suspended (consuming 0 CPU).
    /// Returns Ok(()) when resumed, or Err if the tenant is terminated.
    pub async fn wait_if_suspended(&self, tenant_id: &str) -> Result<(), BillingError> {
        loop {
            let (status, notify) = {
                let state_arc = self.get_or_create_state(tenant_id);
                let state = state_arc.lock().unwrap();
                (state.status, state.notify.clone())
            };

            match status {
                TenantQueueStatus::Active => return Ok(()),
                TenantQueueStatus::Terminated => {
                    return Err(BillingError::TenantTerminated(tenant_id.to_string()))
                }
                TenantQueueStatus::Suspended => {
                    // Wait for resume or termination notification
                    notify.notified().await;
                }
            }
        }
    }

    /// RTE-009: Suspends the tenant's Zenoh queue, dispatches synchronous HTTP webhook
    /// to external billing system, and waits for 200 OK before resuming.
    /// If billing system responds with non-200 or times out, terminates the tenant.
    pub async fn suspend_queue_and_notify_billing(
        &self,
        tenant_id: &str,
        current_spend: f64,
        hard_budget: f64,
        webhook_url: &str,
        timeout: Option<Duration>,
    ) -> Result<BillingOutcome, BillingError> {
        // 1. Physically suspend the Zenoh task queue
        self.suspend_tenant(tenant_id);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let payload = BillingWebhookPayload {
            event: "budget_threshold_reached".to_string(),
            tenant_id: tenant_id.to_string(),
            current_spend,
            hard_budget,
            threshold_percentage: 90.0,
            timestamp: now,
        };

        let req_timeout = timeout.unwrap_or(self.default_timeout);

        println!(
            "[RTE-009] Disagree/freeze: Tenant '{}' reached 90% budget ({}/{}). Emitting synchronous billing webhook to {}...",
            tenant_id, current_spend, hard_budget, webhook_url
        );

        // 2. Emit synchronous HTTP POST request to external billing system
        let send_result = self
            .http_client
            .post(webhook_url)
            .json(&payload)
            .timeout(req_timeout)
            .send()
            .await;

        match send_result {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    // 3. Billing system replied 200 OK: Resume Zenoh queue
                    println!(
                        "[RTE-009] Billing system replied 200 OK for tenant '{}'. Resuming task queue.",
                        tenant_id
                    );
                    self.resume_tenant(tenant_id);
                    Ok(BillingOutcome::Resumed)
                } else {
                    // Non-200 status: Terminate agent
                    println!(
                        "[RTE-009] Billing system rejected request for tenant '{}' with status {}. Terminating agent.",
                        tenant_id, status
                    );
                    self.terminate_tenant(tenant_id);
                    Err(BillingError::BillingRejected {
                        status: status.as_u16(),
                        message: format!("HTTP {}", status),
                    })
                }
            }
            Err(e) => {
                // Timeout or network failure: Terminate agent
                println!(
                    "[RTE-009] Billing webhook failed for tenant '{}': {}. Terminating agent.",
                    tenant_id, e
                );
                self.terminate_tenant(tenant_id);
                if e.is_timeout() {
                    Err(BillingError::Timeout(e.to_string()))
                } else {
                    Err(BillingError::Network(e.to_string()))
                }
            }
        }
    }
}

/// Incoming webhook bridge for long-horizon Gemini interaction status events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractionStatusEvent {
    pub interaction_id: String,
    pub status: String,
    pub usage: Option<serde_json::Value>,
    pub metadata: Option<serde_json::Value>,
}

pub struct WebhookBridge;

impl WebhookBridge {
    /// Deserializes incoming webhook JSON and publishes status to Zenoh topic `interactions/{id}/status`.
    pub async fn handle_interaction_webhook(
        zenoh_session: &zenoh::Session,
        payload_json: &str,
    ) -> Result<InteractionStatusEvent, Box<dyn std::error::Error + Send + Sync>> {
        let event: InteractionStatusEvent = serde_json::from_str(payload_json)?;
        let topic = format!("interactions/{}/status", event.interaction_id);
        let session_payload = serde_json::to_string(&serde_json::json!({
            "interaction_id": event.interaction_id,
            "status": event.status,
            "usage": event.usage,
            "metadata": event.metadata,
        }))?;

        zenoh_session
            .put(topic, session_payload)
            .res_async()
            .await
            .map_err(|e| e.to_string())?;

        Ok(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[tokio::test]
    async fn test_queue_suspension_and_resume_flow() {
        let manager = Arc::new(QueueSuspensionManager::default());
        let tenant = "tenant-test-1";

        assert_eq!(manager.get_status(tenant), TenantQueueStatus::Active);

        // Suspend tenant
        manager.suspend_tenant(tenant);
        assert!(manager.is_suspended(tenant));

        // Spawn a task waiting on the queue
        let manager_clone = manager.clone();
        let unblocked = Arc::new(AtomicBool::new(false));
        let unblocked_clone = unblocked.clone();

        let handle = tokio::spawn(async move {
            let res = manager_clone.wait_if_suspended(tenant).await;
            assert!(res.is_ok());
            unblocked_clone.store(true, Ordering::SeqCst);
        });

        // Small yield to let spawned task wait
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(!unblocked.load(Ordering::SeqCst), "Queue should be frozen");

        // Resume tenant
        manager.resume_tenant(tenant);
        handle.await.unwrap();
        assert!(unblocked.load(Ordering::SeqCst), "Queue should resume");
        assert_eq!(manager.get_status(tenant), TenantQueueStatus::Active);
    }

    #[tokio::test]
    async fn test_queue_suspension_and_termination_flow() {
        let manager = Arc::new(QueueSuspensionManager::default());
        let tenant = "tenant-test-fail";

        manager.suspend_tenant(tenant);

        let manager_clone = manager.clone();
        let handle = tokio::spawn(async move { manager_clone.wait_if_suspended(tenant).await });

        tokio::time::sleep(Duration::from_millis(20)).await;

        // Terminate tenant
        manager.terminate_tenant(tenant);
        let res = handle.await.unwrap();
        assert!(matches!(res, Err(BillingError::TenantTerminated(_))));
        assert_eq!(manager.get_status(tenant), TenantQueueStatus::Terminated);
    }

    #[tokio::test]
    async fn test_sync_billing_webhook_success() {
        // Start a mock HTTP server returning 200 OK
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let webhook_url = format!("http://{}/billing/webhook", addr);

        tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf).await;
                let response = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK";
                let _ = stream.write_all(response.as_bytes()).await;
            }
        });

        let manager = QueueSuspensionManager::new(Duration::from_millis(1000));
        let outcome = manager
            .suspend_queue_and_notify_billing(
                "tenant-billed",
                91.0,
                100.0,
                &webhook_url,
                Some(Duration::from_millis(1000)),
            )
            .await;

        assert_eq!(outcome.unwrap(), BillingOutcome::Resumed);
        assert_eq!(
            manager.get_status("tenant-billed"),
            TenantQueueStatus::Active
        );
    }

    #[tokio::test]
    async fn test_sync_billing_webhook_failure_terminates_agent() {
        // Start a mock HTTP server returning 402 Payment Required
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let webhook_url = format!("http://{}/billing/webhook", addr);

        tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf).await;
                let response = "HTTP/1.1 402 Payment Required\r\nContent-Length: 0\r\n\r\n";
                let _ = stream.write_all(response.as_bytes()).await;
            }
        });

        let manager = QueueSuspensionManager::new(Duration::from_millis(1000));
        let err = manager
            .suspend_queue_and_notify_billing(
                "tenant-unpaid",
                91.0,
                100.0,
                &webhook_url,
                Some(Duration::from_millis(1000)),
            )
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            BillingError::BillingRejected { status: 402, .. }
        ));
        assert_eq!(
            manager.get_status("tenant-unpaid"),
            TenantQueueStatus::Terminated
        );
    }

    #[tokio::test]
    async fn test_sync_billing_webhook_timeout_terminates_agent() {
        // Mock server that never responds
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let webhook_url = format!("http://{}/billing/webhook", addr);

        tokio::spawn(async move {
            if let Ok((_stream, _)) = listener.accept().await {
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        });

        let manager = QueueSuspensionManager::new(Duration::from_millis(100));
        let err = manager
            .suspend_queue_and_notify_billing(
                "tenant-timeout",
                91.0,
                100.0,
                &webhook_url,
                Some(Duration::from_millis(100)),
            )
            .await
            .unwrap_err();

        assert!(matches!(err, BillingError::Timeout(_)));
        assert_eq!(
            manager.get_status("tenant-timeout"),
            TenantQueueStatus::Terminated
        );
    }
}
