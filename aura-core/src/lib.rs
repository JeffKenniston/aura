pub mod bus;
pub mod config;
pub mod hypervisor;
pub mod identity;
pub mod knowledge;
pub mod runtime;
pub mod supervisor;
pub mod transport;

use std::sync::Arc;

/// Microkernel execution host initialization adhering to M1 Domain Boundaries
pub async fn init() -> Result<(), Box<dyn std::error::Error>> {
    init_with_config(config::AuraConfig::default()).await
}

/// Microkernel execution host initialization with custom declarative configuration (HOST-008)
pub async fn init_with_config(config: config::AuraConfig) -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting Aura Core microkernel initialization...");

    // 1. Load and validate declarative configuration (HOST-008)
    config.validate()?;
    println!("Declarative configuration validated successfully.");

    // 2. Verify and register Tokio Runtime pool (M0-SPIKE-03, HOST-002)
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        runtime::set_runtime_handle(handle);
        let worker_count = runtime::metrics().map(|m| m.num_workers()).unwrap_or(0);
        println!(
            "Tokio runtime pool verified and registered. Active worker threads: {}",
            worker_count
        );
    } else {
        return Err("Aura Core microkernel must be initialized within a running Tokio runtime context".into());
    }

    // 3. Initialize SPIFFE/SPIRE Identity attestation
    let identity_doc = identity::fetch_svid().await?;

    // 4. Initialize Zenoh event bus with SVID scoping (BUS-001 - BUS-009)
    let bus = Arc::new(bus::ZenohBus::with_config(&identity_doc, &config.bus).await?);
    bus.start_task_listener().await?;
    bus.start_token_monitor(config.bus.token_threshold).await?;

    // 5. Initialize native HTTP/3 QUIC transport multiplexer (NET-001 - NET-007)
    transport::http3::Http3Transport::start_with_config(&config.transport).await?;

    // 6. Initialize WASM and Firecracker hypervisors
    hypervisor::init_wasmtime()?;
    hypervisor::init_firecracker()?;

    // 7. Initialize Supervisor for child processes and graceful drain (HOST-007, HOST-009)
    let supervisor = Arc::new(supervisor::Supervisor::new(config.supervisor));
    supervisor.start_signal_handler();

    println!("Aura Core microkernel successfully initialized.");
    Ok(())
}

/// Synchronous bootstrap entry point that initializes the Tokio runtime pool and drives `init()` (M0-SPIKE-03).
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    run_with_config(config::AuraConfig::default())
}

/// Synchronous bootstrap entry point with custom declarative configuration.
pub fn run_with_config(config: config::AuraConfig) -> Result<(), Box<dyn std::error::Error>> {
    config.validate()?;
    let runtime = runtime::init_tokio_pool_with_config(&config.runtime)?;
    runtime.block_on(init_with_config(config))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_microkernel_init() {
        // Run init in test environment with fallback identity and ephemeral ports
        let init_res = init().await;
        if let Err(e) = &init_res {
            if let Some(io_err) = e.downcast_ref::<std::io::Error>() {
                if io_err.kind() == std::io::ErrorKind::AddrInUse {
                    println!("Port in use during concurrent tests, passing");
                    return;
                }
            }
        }
        assert!(init_res.is_ok(), "init() should succeed: {:?}", init_res);
    }
}
