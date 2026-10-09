use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    TomlParse(toml::de::Error),
    YamlParse(serde_yaml::Error),
    Validation(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Io(e) => write!(f, "I/O error reading configuration: {}", e),
            ConfigError::TomlParse(e) => write!(f, "TOML parse error: {}", e),
            ConfigError::YamlParse(e) => write!(f, "YAML parse error: {}", e),
            ConfigError::Validation(e) => {
                write!(f, "Configuration schema validation failed: {}", e)
            }
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConfigError::Io(e) => Some(e),
            ConfigError::TomlParse(e) => Some(e),
            ConfigError::YamlParse(e) => Some(e),
            ConfigError::Validation(_) => None,
        }
    }
}

impl From<std::io::Error> for ConfigError {
    fn from(err: std::io::Error) -> Self {
        ConfigError::Io(err)
    }
}

impl From<toml::de::Error> for ConfigError {
    fn from(err: toml::de::Error) -> Self {
        ConfigError::TomlParse(err)
    }
}

impl From<serde_yaml::Error> for ConfigError {
    fn from(err: serde_yaml::Error) -> Self {
        ConfigError::YamlParse(err)
    }
}

/// Root configuration schema for Aura OS microkernel (HOST-008).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AuraConfig {
    #[serde(default)]
    pub kernel: KernelConfig,
    #[serde(default)]
    pub bus: BusConfig,
    #[serde(default)]
    pub transport: TransportConfig,
    #[serde(default)]
    pub hypervisor: HypervisorConfig,
    #[serde(default)]
    pub supervisor: SupervisorConfig,
    #[serde(default)]
    pub pricing: PricingConfig,
}

impl AuraConfig {
    /// Validates the declarative configuration against microkernel invariants.
    pub fn validate(&self) -> Result<(), ConfigError> {
        self.kernel.validate()?;
        self.bus.validate()?;
        self.transport.validate()?;
        self.hypervisor.validate()?;
        self.supervisor.validate()?;
        self.pricing.validate()?;
        Ok(())
    }

    /// Loads and validates configuration from a TOML string.
    pub fn from_toml_str(content: &str) -> Result<Self, ConfigError> {
        let config: AuraConfig = toml::from_str(content)?;
        config.validate()?;
        Ok(config)
    }

    /// Loads and validates configuration from a YAML string.
    pub fn from_yaml_str(content: &str) -> Result<Self, ConfigError> {
        let config: AuraConfig = serde_yaml::from_str(content)?;
        config.validate()?;
        Ok(config)
    }

    /// Loads and validates configuration from a file (auto-detecting format from extension).
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let path_ref = path.as_ref();
        let content = fs::read_to_string(path_ref)?;
        let extension = path_ref
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("toml");

        match extension {
            "yaml" | "yml" => Self::from_yaml_str(&content),
            _ => Self::from_toml_str(&content),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KernelConfig {
    pub name: String,
    pub environment: String,
    pub spiffe_socket_path: String,
}

impl Default for KernelConfig {
    fn default() -> Self {
        Self {
            name: "aura-core".to_string(),
            environment: "development".to_string(),
            spiffe_socket_path: "unix:///run/spire/sockets/agent.sock".to_string(),
        }
    }
}

impl KernelConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.name.trim().is_empty() {
            return Err(ConfigError::Validation(
                "kernel.name cannot be empty".into(),
            ));
        }
        if self.environment.trim().is_empty() {
            return Err(ConfigError::Validation(
                "kernel.environment cannot be empty".into(),
            ));
        }
        if self.spiffe_socket_path.trim().is_empty() {
            return Err(ConfigError::Validation(
                "kernel.spiffe_socket_path cannot be empty".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BusConfig {
    pub prefix: String,
    pub rate_limit_capacity: u32,
    pub rate_limit_refill_amount: u32,
    pub rate_limit_refill_rate_ms: u64,
    pub token_threshold: u32,
}

impl Default for BusConfig {
    fn default() -> Self {
        Self {
            prefix: "aura/workspace".to_string(),
            rate_limit_capacity: 100,
            rate_limit_refill_amount: 10,
            rate_limit_refill_rate_ms: 1000,
            token_threshold: 800_000,
        }
    }
}

impl BusConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.prefix.trim().is_empty() {
            return Err(ConfigError::Validation("bus.prefix cannot be empty".into()));
        }
        if self.rate_limit_capacity == 0 {
            return Err(ConfigError::Validation(
                "bus.rate_limit_capacity must be greater than 0".into(),
            ));
        }
        if self.rate_limit_refill_amount == 0 {
            return Err(ConfigError::Validation(
                "bus.rate_limit_refill_amount must be greater than 0".into(),
            ));
        }
        if self.rate_limit_refill_rate_ms == 0 {
            return Err(ConfigError::Validation(
                "bus.rate_limit_refill_rate_ms must be greater than 0".into(),
            ));
        }
        if self.token_threshold == 0 {
            return Err(ConfigError::Validation(
                "bus.token_threshold must be greater than 0".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TransportConfig {
    pub udp_bind_addr: String,
    pub tcp_bind_addr: String,
    pub alt_svc_header: String,
    pub max_idle_timeout_ms: u64,
    pub max_udp_payload_size: usize,
    pub initial_max_data: u64,
    pub initial_max_streams_bidi: u64,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            udp_bind_addr: "127.0.0.1:4433".to_string(),
            tcp_bind_addr: "127.0.0.1:4433".to_string(),
            alt_svc_header: "h3=\":4433\"; ma=86400".to_string(),
            max_idle_timeout_ms: 5000,
            max_udp_payload_size: 1350,
            initial_max_data: 10_000_000,
            initial_max_streams_bidi: 100,
        }
    }
}

impl TransportConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        self.udp_bind_addr.parse::<SocketAddr>().map_err(|e| {
            ConfigError::Validation(format!("Invalid transport.udp_bind_addr: {}", e))
        })?;
        self.tcp_bind_addr.parse::<SocketAddr>().map_err(|e| {
            ConfigError::Validation(format!("Invalid transport.tcp_bind_addr: {}", e))
        })?;
        if self.max_udp_payload_size < 1200 {
            return Err(ConfigError::Validation(
                "transport.max_udp_payload_size must be at least 1200 bytes for QUIC".into(),
            ));
        }
        if self.max_idle_timeout_ms == 0 {
            return Err(ConfigError::Validation(
                "transport.max_idle_timeout_ms must be greater than 0".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct HypervisorConfig {
    #[serde(default)]
    pub wasm: WasmConfig,
    #[serde(default)]
    pub firecracker: FirecrackerConfig,
    #[serde(default)]
    pub proxy: crate::hypervisor::proxy::EgressProxyConfig,
}

impl HypervisorConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        self.wasm.validate()?;
        self.firecracker.validate()?;
        self.proxy.validate().map_err(ConfigError::Validation)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WasmConfig {
    pub default_fuel_budget: u64,
    pub pooling_allocator: bool,
    pub opt_level: String,
}

impl Default for WasmConfig {
    fn default() -> Self {
        Self {
            default_fuel_budget: 10_000,
            pooling_allocator: true,
            opt_level: "speed".to_string(),
        }
    }
}

impl WasmConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.default_fuel_budget == 0 {
            return Err(ConfigError::Validation(
                "hypervisor.wasm.default_fuel_budget must be greater than 0".into(),
            ));
        }
        match self.opt_level.as_str() {
            "none" | "speed" | "size" => Ok(()),
            other => Err(ConfigError::Validation(format!(
                "Invalid hypervisor.wasm.opt_level: '{}'. Must be 'none', 'speed', or 'size'",
                other
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FirecrackerConfig {
    pub jailer_binary: PathBuf,
    pub firecracker_binary: PathBuf,
    pub chroot_base_dir: PathBuf,
    pub uid: u32,
    pub gid: u32,
    pub default_vcpu_count: u32,
    pub default_mem_size_mib: u32,
}

impl Default for FirecrackerConfig {
    fn default() -> Self {
        Self {
            jailer_binary: PathBuf::from("/usr/bin/jailer"),
            firecracker_binary: PathBuf::from("/usr/local/bin/firecracker"),
            chroot_base_dir: PathBuf::from("/srv/jailer"),
            uid: 1000,
            gid: 1000,
            default_vcpu_count: 1,
            default_mem_size_mib: 128,
        }
    }
}

impl FirecrackerConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.default_vcpu_count == 0 {
            return Err(ConfigError::Validation(
                "hypervisor.firecracker.default_vcpu_count must be at least 1".into(),
            ));
        }
        if self.default_mem_size_mib < 16 {
            return Err(ConfigError::Validation(
                "hypervisor.firecracker.default_mem_size_mib must be at least 16 MiB".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SupervisorConfig {
    pub graceful_shutdown_timeout_secs: u64,
    pub drain_timeout_secs: u64,
    pub health_check_interval_ms: u64,
}

impl Default for SupervisorConfig {
    fn default() -> Self {
        Self {
            graceful_shutdown_timeout_secs: 15,
            drain_timeout_secs: 10,
            health_check_interval_ms: 1000,
        }
    }
}

impl SupervisorConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.graceful_shutdown_timeout_secs == 0 {
            return Err(ConfigError::Validation(
                "supervisor.graceful_shutdown_timeout_secs must be greater than 0".into(),
            ));
        }
        if self.drain_timeout_secs == 0 {
            return Err(ConfigError::Validation(
                "supervisor.drain_timeout_secs must be greater than 0".into(),
            ));
        }
        if self.health_check_interval_ms == 0 {
            return Err(ConfigError::Validation(
                "supervisor.health_check_interval_ms must be greater than 0".into(),
            ));
        }
        Ok(())
    }
}

/// Pricing and billing configuration for token usage and file storage (COG-014, RTE-009).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PricingConfig {
    /// Cost per million input tokens in USD.
    #[serde(default = "default_input_token_cost_per_million")]
    pub input_token_cost_per_million: f64,

    /// Cost per million output tokens in USD.
    #[serde(default = "default_output_token_cost_per_million")]
    pub output_token_cost_per_million: f64,

    /// File storage cost per GB per hour in USD (COG-014).
    #[serde(default = "default_file_storage_cost_per_gb_hour")]
    pub file_storage_cost_per_gb_hour: f64,

    /// File storage cost per GB per month in USD (default $0.02/GB-month).
    #[serde(default = "default_file_storage_cost_per_gb_month")]
    pub file_storage_cost_per_gb_month: f64,

    /// Hard cost budget in USD (or token units) before queue suspension (RTE-009).
    #[serde(default = "default_hard_cost_budget")]
    pub hard_cost_budget: f64,

    /// Threshold fraction of hard cost budget triggering queue suspension (default: 0.90 = 90%).
    #[serde(default = "default_budget_suspension_threshold")]
    pub budget_suspension_threshold: f64,

    /// Billing system webhook URL to notify when 90% hard cost budget is reached (RTE-009).
    #[serde(default = "default_billing_webhook_url")]
    pub billing_webhook_url: String,

    /// Timeout for billing webhook request in milliseconds.
    #[serde(default = "default_webhook_timeout_ms")]
    pub webhook_timeout_ms: u64,
}

fn default_input_token_cost_per_million() -> f64 {
    0.15
}

fn default_output_token_cost_per_million() -> f64 {
    0.60
}

fn default_file_storage_cost_per_gb_hour() -> f64 {
    0.02 / 720.0
}

fn default_file_storage_cost_per_gb_month() -> f64 {
    0.02
}

fn default_hard_cost_budget() -> f64 {
    100.0
}

fn default_budget_suspension_threshold() -> f64 {
    0.90
}

fn default_billing_webhook_url() -> String {
    "http://127.0.0.1:8080/billing/webhook".to_string()
}

fn default_webhook_timeout_ms() -> u64 {
    5000
}

impl Default for PricingConfig {
    fn default() -> Self {
        Self {
            input_token_cost_per_million: default_input_token_cost_per_million(),
            output_token_cost_per_million: default_output_token_cost_per_million(),
            file_storage_cost_per_gb_hour: default_file_storage_cost_per_gb_hour(),
            file_storage_cost_per_gb_month: default_file_storage_cost_per_gb_month(),
            hard_cost_budget: default_hard_cost_budget(),
            budget_suspension_threshold: default_budget_suspension_threshold(),
            billing_webhook_url: default_billing_webhook_url(),
            webhook_timeout_ms: default_webhook_timeout_ms(),
        }
    }
}

impl PricingConfig {
    /// Calculates Gemini API token cost in USD based on input and output token counts.
    pub fn calculate_token_cost(&self, input_tokens: u64, output_tokens: u64) -> f64 {
        (input_tokens as f64 * self.input_token_cost_per_million / 1_000_000.0)
            + (output_tokens as f64 * self.output_token_cost_per_million / 1_000_000.0)
    }

    /// Calculates storage cost for bytes held over duration in hours.
    pub fn calculate_storage_cost_gb_hours(&self, size_bytes: u64, duration_hours: f64) -> f64 {
        let size_gb = size_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
        size_gb * duration_hours * self.file_storage_cost_per_gb_hour
    }

    /// Calculates storage cost for bytes held over TTL days using monthly rate (COG-014).
    pub fn calculate_storage_cost_by_days(&self, size_bytes: u64, ttl_days: f64) -> f64 {
        let size_gb = size_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
        let month_fraction = ttl_days / 30.0;
        size_gb * month_fraction * self.file_storage_cost_per_gb_month
    }

    /// Returns true if current spend or token count has reached the 90% hard cost budget threshold.
    pub fn is_suspension_threshold_reached(&self, current_spend: f64) -> bool {
        current_spend >= (self.hard_cost_budget * self.budget_suspension_threshold)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.input_token_cost_per_million < 0.0 {
            return Err(ConfigError::Validation(
                "pricing.input_token_cost_per_million must be non-negative".into(),
            ));
        }
        if self.output_token_cost_per_million < 0.0 {
            return Err(ConfigError::Validation(
                "pricing.output_token_cost_per_million must be non-negative".into(),
            ));
        }
        if self.file_storage_cost_per_gb_hour < 0.0 {
            return Err(ConfigError::Validation(
                "pricing.file_storage_cost_per_gb_hour must be non-negative".into(),
            ));
        }
        if self.file_storage_cost_per_gb_month < 0.0 {
            return Err(ConfigError::Validation(
                "pricing.file_storage_cost_per_gb_month must be non-negative".into(),
            ));
        }
        if self.hard_cost_budget <= 0.0 {
            return Err(ConfigError::Validation(
                "pricing.hard_cost_budget must be greater than 0".into(),
            ));
        }
        if self.budget_suspension_threshold <= 0.0 || self.budget_suspension_threshold > 1.0 {
            return Err(ConfigError::Validation(
                "pricing.budget_suspension_threshold must be between 0.0 and 1.0".into(),
            ));
        }
        if self.billing_webhook_url.trim().is_empty() {
            return Err(ConfigError::Validation(
                "pricing.billing_webhook_url cannot be empty".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_valid() {
        let config = AuraConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_toml_deserialization_and_validation() {
        let toml_data = r#"
            [kernel]
            name = "aura-test"
            environment = "testing"
            spiffe_socket_path = "unix:///tmp/spire.sock"

            [bus]
            prefix = "aura/test"
            rate_limit_capacity = 50
            rate_limit_refill_amount = 5
            rate_limit_refill_rate_ms = 500
            token_threshold = 400000

            [transport]
            udp_bind_addr = "127.0.0.1:8443"
            tcp_bind_addr = "127.0.0.1:8443"
            alt_svc_header = "h3=\":8443\"; ma=86400"
            max_idle_timeout_ms = 10000
            max_udp_payload_size = 1400
            initial_max_data = 5000000
            initial_max_streams_bidi = 50

            [hypervisor.wasm]
            default_fuel_budget = 20000
            pooling_allocator = true
            opt_level = "speed"

            [hypervisor.firecracker]
            jailer_binary = "/usr/bin/jailer"
            firecracker_binary = "/usr/bin/firecracker"
            chroot_base_dir = "/srv/jailer"
            uid = 1001
            gid = 1001
            default_vcpu_count = 2
            default_mem_size_mib = 256

            [supervisor]
            graceful_shutdown_timeout_secs = 20
            drain_timeout_secs = 15
            health_check_interval_ms = 500
        "#;

        let config = AuraConfig::from_toml_str(toml_data).expect("Failed to parse TOML");
        assert_eq!(config.kernel.name, "aura-test");
        assert_eq!(config.bus.rate_limit_capacity, 50);
        assert_eq!(config.transport.udp_bind_addr, "127.0.0.1:8443");
        assert_eq!(config.hypervisor.wasm.default_fuel_budget, 20000);
        assert_eq!(config.hypervisor.firecracker.default_vcpu_count, 2);
        assert_eq!(config.supervisor.drain_timeout_secs, 15);
    }

    #[test]
    fn test_yaml_deserialization_and_validation() {
        let yaml_data = r#"
kernel:
  name: "aura-yaml"
  environment: "staging"
  spiffe_socket_path: "unix:///tmp/spire.sock"
bus:
  prefix: "aura/staging"
  rate_limit_capacity: 75
  rate_limit_refill_amount: 15
  rate_limit_refill_rate_ms: 1000
  token_threshold: 600000
transport:
  udp_bind_addr: "127.0.0.1:9443"
  tcp_bind_addr: "127.0.0.1:9443"
  alt_svc_header: 'h3=":9443"; ma=86400'
  max_idle_timeout_ms: 6000
  max_udp_payload_size: 1350
  initial_max_data: 8000000
  initial_max_streams_bidi: 80
hypervisor:
  wasm:
    default_fuel_budget: 15000
    pooling_allocator: true
    opt_level: "size"
  firecracker:
    jailer_binary: "/usr/bin/jailer"
    firecracker_binary: "/usr/bin/firecracker"
    chroot_base_dir: "/srv/jailer"
    uid: 1000
    gid: 1000
    default_vcpu_count: 1
    default_mem_size_mib: 128
supervisor:
  graceful_shutdown_timeout_secs: 10
  drain_timeout_secs: 5
  health_check_interval_ms: 1000
"#;

        let config = AuraConfig::from_yaml_str(yaml_data).expect("Failed to parse YAML");
        assert_eq!(config.kernel.name, "aura-yaml");
        assert_eq!(config.bus.rate_limit_capacity, 75);
        assert_eq!(config.transport.udp_bind_addr, "127.0.0.1:9443");
        assert_eq!(config.hypervisor.wasm.opt_level, "size");
    }

    #[test]
    fn test_validation_rejects_invalid_addr() {
        let mut config = AuraConfig::default();
        config.transport.udp_bind_addr = "invalid-address".to_string();
        let err = config.validate().unwrap_err();
        assert!(matches!(err, ConfigError::Validation(_)));
        assert!(err.to_string().contains("transport.udp_bind_addr"));
    }

    #[test]
    fn test_validation_rejects_zero_fuel() {
        let mut config = AuraConfig::default();
        config.hypervisor.wasm.default_fuel_budget = 0;
        let err = config.validate().unwrap_err();
        assert!(matches!(err, ConfigError::Validation(_)));
        assert!(err.to_string().contains("default_fuel_budget"));
    }

    #[test]
    fn test_validation_rejects_zero_rate_limit() {
        let mut config = AuraConfig::default();
        config.bus.rate_limit_capacity = 0;
        let err = config.validate().unwrap_err();
        assert!(matches!(err, ConfigError::Validation(_)));
        assert!(err.to_string().contains("rate_limit_capacity"));
    }

    #[test]
    fn test_validation_rejects_invalid_wasm_opt_level() {
        let mut config = AuraConfig::default();
        config.hypervisor.wasm.opt_level = "ultra_fast".to_string();
        let err = config.validate().unwrap_err();
        assert!(matches!(err, ConfigError::Validation(_)));
        assert!(err.to_string().contains("opt_level"));
    }

    #[test]
    fn test_validation_rejects_empty_proxy_allowlist() {
        let mut config = AuraConfig::default();
        config.hypervisor.proxy.allowed_domains = vec![];
        let err = config.validate().unwrap_err();
        assert!(matches!(err, ConfigError::Validation(_)));
        assert!(err.to_string().contains("allowed_domains"));
    }

    #[test]
    fn test_pricing_config_calculations() {
        let pricing = PricingConfig::default();
        // 1M input tokens at $0.15 + 1M output tokens at $0.60 = $0.75
        let token_cost = pricing.calculate_token_cost(1_000_000, 1_000_000);
        assert!((token_cost - 0.75).abs() < 1e-6);

        // 1 GB file for 30 days at $0.02/GB-month
        let storage_cost = pricing.calculate_storage_cost_by_days(1024 * 1024 * 1024, 30.0);
        assert!((storage_cost - 0.02).abs() < 1e-6);

        // Check 90% threshold
        assert!(!pricing.is_suspension_threshold_reached(89.0));
        assert!(pricing.is_suspension_threshold_reached(90.0));
        assert!(pricing.is_suspension_threshold_reached(91.0));
    }

    #[test]
    fn test_pricing_config_toml_parsing() {
        let toml_str = r#"
            [pricing]
            input_token_cost_per_million = 0.075
            output_token_cost_per_million = 0.30
            file_storage_cost_per_gb_hour = 0.00003
            file_storage_cost_per_gb_month = 0.02
            hard_cost_budget = 50.0
            budget_suspension_threshold = 0.90
            billing_webhook_url = "http://billing.internal/webhook"
        "#;
        let config =
            AuraConfig::from_toml_str(toml_str).expect("Failed to parse TOML with pricing");
        assert_eq!(config.pricing.input_token_cost_per_million, 0.075);
        assert_eq!(config.pricing.output_token_cost_per_million, 0.30);
        assert_eq!(config.pricing.hard_cost_budget, 50.0);
        assert_eq!(
            config.pricing.billing_webhook_url,
            "http://billing.internal/webhook"
        );
    }

    #[test]
    fn test_pricing_config_validation_rejects_negative_or_zero() {
        let mut config = AuraConfig::default();
        config.pricing.input_token_cost_per_million = -1.0;
        assert!(config.validate().is_err());

        let mut config2 = AuraConfig::default();
        config2.pricing.hard_cost_budget = 0.0;
        assert!(config2.validate().is_err());

        let mut config3 = AuraConfig::default();
        config3.pricing.budget_suspension_threshold = 1.5;
        assert!(config3.validate().is_err());
    }
}
