use crate::config::PricingConfig;
use crate::transport::webhook::{BillingError, BillingOutcome, QueueSuspensionManager};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

struct TokenBucket {
    tokens: u32,
    last_refill: Instant,
}

/// Token Bucket rate limiter for managing event bus backpressure (BUS-008, ADR-001).
pub struct RateLimiter {
    buckets: DashMap<String, TokenBucket>,
    capacity: u32,
    refill_rate: Duration,
    refill_amount: u32,
}

impl RateLimiter {
    pub fn new(capacity: u32, refill_amount: u32, refill_rate: Duration) -> Self {
        Self {
            buckets: DashMap::new(),
            capacity,
            refill_amount,
            refill_rate,
        }
    }

    /// Checks if a client with the given SPIFFE ID is allowed to consume a token.
    /// Returns true if allowed, or false if rate limit exceeded.
    pub fn check_and_consume(&self, spiffe_id: &str) -> bool {
        let mut bucket = self
            .buckets
            .entry(spiffe_id.to_string())
            .or_insert_with(|| TokenBucket {
                tokens: self.capacity,
                last_refill: Instant::now(),
            });

        let now = Instant::now();
        let elapsed = now.duration_since(bucket.last_refill);

        if elapsed >= self.refill_rate {
            let refills = (elapsed.as_millis() / self.refill_rate.as_millis()) as u32;
            let new_tokens = bucket.tokens + refills * self.refill_amount;
            bucket.tokens = new_tokens.min(self.capacity);
            bucket.last_refill = now;
        }

        if bucket.tokens > 0 {
            bucket.tokens -= 1;
            true
        } else {
            false
        }
    }

    /// Resets the rate limit bucket for a specific SPIFFE ID.
    pub fn reset(&self, spiffe_id: &str) {
        self.buckets.remove(spiffe_id);
    }
}

/// Metadata for a tracked Gemini API file search store file (COG-014).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedFile {
    pub file_id: String,
    pub tenant_id: String,
    pub size_bytes: u64,
    pub ttl_days: f64,
    pub created_at: u64,
}

/// Aggregate usage and cost metrics per tenant.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TenantUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub token_cost: f64,
    pub storage_cost: f64,
    pub total_cost: f64,
}

/// Shadow tracking counter monitoring aggregate session tokens, file storage costs,
/// and budget thresholds (COG-014, RTE-009).
pub struct ShadowTracker {
    pricing: RwLock<PricingConfig>,
    tracked_files: DashMap<String, DashMap<String, TrackedFile>>,
    tenant_usages: DashMap<String, TenantUsage>,
    suspension_mgr: Option<Arc<QueueSuspensionManager>>,
}

impl ShadowTracker {
    pub fn new(pricing: PricingConfig) -> Self {
        Self {
            pricing: RwLock::new(pricing),
            tracked_files: DashMap::new(),
            tenant_usages: DashMap::new(),
            suspension_mgr: None,
        }
    }

    pub fn with_suspension_manager(
        pricing: PricingConfig,
        suspension_mgr: Arc<QueueSuspensionManager>,
    ) -> Self {
        Self {
            pricing: RwLock::new(pricing),
            tracked_files: DashMap::new(),
            tenant_usages: DashMap::new(),
            suspension_mgr: Some(suspension_mgr),
        }
    }

    /// Updates dynamic PricingConfig at runtime.
    pub fn set_pricing(&self, pricing: PricingConfig) {
        let mut p = self.pricing.write().unwrap();
        *p = pricing;
    }

    /// Tracks an uploaded file for Gemini file search stores (COG-014).
    pub fn register_file(&self, tenant_id: &str, file_id: &str, size_bytes: u64, ttl_days: f64) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let tracked = TrackedFile {
            file_id: file_id.to_string(),
            tenant_id: tenant_id.to_string(),
            size_bytes,
            ttl_days,
            created_at: now,
        };

        {
            let files = self.tracked_files.entry(tenant_id.to_string()).or_default();
            files.insert(file_id.to_string(), tracked);
        }

        self.recalculate_tenant_costs(tenant_id);
    }

    /// Removes a file from tracking upon deletion.
    pub fn delete_file(&self, tenant_id: &str, file_id: &str) -> bool {
        let removed = if let Some(files) = self.tracked_files.get(tenant_id) {
            files.remove(file_id).is_some()
        } else {
            false
        };

        if removed {
            self.recalculate_tenant_costs(tenant_id);
        }
        removed
    }

    /// Calculates cumulative Gemini API file storage costs for a tenant (COG-014).
    /// Correlates uploaded object sizes and TTLs against PricingConfig.
    pub fn calculate_file_storage_costs(&self, tenant_id: &str) -> f64 {
        let pricing = self.pricing.read().unwrap();
        let mut total_storage_cost = 0.0;

        if let Some(files) = self.tracked_files.get(tenant_id) {
            for file_entry in files.iter() {
                let file = file_entry.value();
                let cost = pricing.calculate_storage_cost_by_days(file.size_bytes, file.ttl_days);
                total_storage_cost += cost;
            }
        }

        (total_storage_cost * 1_000_000.0).round() / 1_000_000.0
    }

    /// Records token usage for a tenant and updates cumulative cost.
    pub fn record_tokens(&self, tenant_id: &str, input_tokens: u64, output_tokens: u64) {
        let pricing = self.pricing.read().unwrap();
        let cost = pricing.calculate_token_cost(input_tokens, output_tokens);

        let mut usage = self.tenant_usages.entry(tenant_id.to_string()).or_default();

        usage.input_tokens += input_tokens;
        usage.output_tokens += output_tokens;
        usage.total_tokens += input_tokens + output_tokens;
        usage.token_cost += cost;
        usage.total_cost = usage.token_cost + usage.storage_cost;
    }

    /// Records aggregate total tokens (e.g. from token telemetry events).
    pub fn record_metric_tokens(&self, tenant_id: &str, total_tokens: u64) {
        let pricing = self.pricing.read().unwrap();
        // Assuming default 1:1 input/output or input pricing if unspecified
        let cost = pricing.calculate_token_cost(total_tokens, 0);

        let mut usage = self.tenant_usages.entry(tenant_id.to_string()).or_default();

        usage.total_tokens = total_tokens;
        usage.token_cost = cost;
        usage.total_cost = usage.token_cost + usage.storage_cost;
    }

    fn recalculate_tenant_costs(&self, tenant_id: &str) {
        let storage_cost = self.calculate_file_storage_costs(tenant_id);
        let mut usage = self.tenant_usages.entry(tenant_id.to_string()).or_default();
        usage.storage_cost = storage_cost;
        usage.total_cost = usage.token_cost + storage_cost;
    }

    /// Returns current cumulative usage and costs for a tenant.
    pub fn get_usage(&self, tenant_id: &str) -> TenantUsage {
        self.tenant_usages
            .get(tenant_id)
            .map(|u| u.clone())
            .unwrap_or_default()
    }

    /// Checks if a tenant has reached 90% hard cost budget.
    /// Supports both dollar spend and token-denominated test budgets.
    pub fn check_budget_threshold(&self, tenant_id: &str) -> (bool, f64, f64) {
        let pricing = self.pricing.read().unwrap();
        let usage = self.get_usage(tenant_id);

        let hard_budget = pricing.hard_cost_budget;
        let threshold_fraction = pricing.budget_suspension_threshold;
        let threshold_value = hard_budget * threshold_fraction;

        // Check 1: Dollar cost budget
        if usage.total_cost >= threshold_value {
            return (true, usage.total_cost, hard_budget);
        }

        // Check 2: Token-denominated budget (when hard_cost_budget is set as token count, e.g. 100)
        if (usage.total_tokens as f64) >= threshold_value {
            return (true, usage.total_tokens as f64, hard_budget);
        }

        (false, usage.total_cost, hard_budget)
    }

    /// RTE-009: Checks budget; if 90% threshold is reached, invokes the suspension manager
    /// to suspend the Zenoh task queue and emit the synchronous billing webhook.
    pub async fn check_and_suspend_if_needed(
        &self,
        tenant_id: &str,
    ) -> Result<Option<BillingOutcome>, BillingError> {
        let (exceeded, current_spend, hard_budget) = self.check_budget_threshold(tenant_id);

        if exceeded {
            if let Some(ref mgr) = self.suspension_mgr {
                let (webhook_url, timeout) = {
                    let pricing = self.pricing.read().unwrap();
                    (
                        pricing.billing_webhook_url.clone(),
                        Duration::from_millis(pricing.webhook_timeout_ms),
                    )
                };

                let outcome = mgr
                    .suspend_queue_and_notify_billing(
                        tenant_id,
                        current_spend,
                        hard_budget,
                        &webhook_url,
                        Some(timeout),
                    )
                    .await?;

                return Ok(Some(outcome));
            }
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limiter() {
        let limiter = RateLimiter::new(2, 1, Duration::from_millis(100));
        assert!(limiter.check_and_consume("spiffe://test"));
        assert!(limiter.check_and_consume("spiffe://test"));
        assert!(!limiter.check_and_consume("spiffe://test"));
        std::thread::sleep(Duration::from_millis(110));
        assert!(limiter.check_and_consume("spiffe://test"));
        assert!(!limiter.check_and_consume("spiffe://test"));
    }

    #[test]
    fn test_cog_014_file_storage_costs_calculation() {
        let mut pricing = PricingConfig::default();
        pricing.file_storage_cost_per_gb_month = 0.02;

        let tracker = ShadowTracker::new(pricing);
        let tenant = "tenant-storage-1";

        // 1 GB file for 30 days at $0.02/GB-month
        let one_gb = 1024 * 1024 * 1024;
        tracker.register_file(tenant, "file-1", one_gb, 30.0);

        let cost = tracker.calculate_file_storage_costs(tenant);
        assert!((cost - 0.02).abs() < 1e-4, "Expected $0.02, got {}", cost);

        // Add second file: 512 MB for 15 days => 0.5 GB * 0.5 month * 0.02 = 0.005
        let half_gb = 512 * 1024 * 1024;
        tracker.register_file(tenant, "file-2", half_gb, 15.0);

        let total_cost = tracker.calculate_file_storage_costs(tenant);
        assert!(
            (total_cost - 0.025).abs() < 1e-4,
            "Expected $0.025, got {}",
            total_cost
        );

        // Delete file-1
        assert!(tracker.delete_file(tenant, "file-1"));
        let remaining_cost = tracker.calculate_file_storage_costs(tenant);
        assert!(
            (remaining_cost - 0.005).abs() < 1e-4,
            "Expected $0.005, got {}",
            remaining_cost
        );
    }

    #[tokio::test]
    async fn test_90_percent_budget_freeze_and_resume() {
        // Mock billing server
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

        let mut pricing = PricingConfig::default();
        pricing.hard_cost_budget = 100.0;
        pricing.budget_suspension_threshold = 0.90;
        pricing.billing_webhook_url = webhook_url;
        pricing.webhook_timeout_ms = 1000;

        let suspension_mgr = Arc::new(QueueSuspensionManager::new(Duration::from_millis(1000)));
        let tracker = ShadowTracker::with_suspension_manager(pricing, suspension_mgr.clone());
        let tenant = "tenant-frozen";

        // Push shadow tracker to 91 tokens (91% of 100 token budget)
        tracker.record_metric_tokens(tenant, 91);

        let (exceeded, spend, budget) = tracker.check_budget_threshold(tenant);
        assert!(exceeded);
        assert_eq!(spend, 91.0);
        assert_eq!(budget, 100.0);

        // Trigger check and suspend
        let outcome = tracker
            .check_and_suspend_if_needed(tenant)
            .await
            .expect("Webhook should succeed with 200 OK");

        assert_eq!(outcome, Some(BillingOutcome::Resumed));
        assert_eq!(
            suspension_mgr.get_status(tenant),
            crate::transport::webhook::TenantQueueStatus::Active
        );
    }
}
