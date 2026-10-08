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
}

impl AuraConfig {
    /// Validates the declarative configuration against microkernel invariants.
    pub fn validate(&self) -> Result<(), ConfigError> {
        self.kernel.validate()?;
        self.bus.validate()?;
        self.transport.validate()?;
        self.hypervisor.validate()?;
        self.supervisor.validate()?;
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
}

impl HypervisorConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        self.wasm.validate()?;
        self.firecracker.validate()?;
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
}
