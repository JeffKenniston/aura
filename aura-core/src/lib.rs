pub mod hypervisor;
pub mod identity;
pub mod runtime;
pub mod transport;
pub mod knowledge;

/// Microkernel execution host initialization
pub async fn init() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting Aura Core microkernel initialization...");

    // 1. Initialize Runtime
    runtime::init_tokio_pool()?;
    
    // 2. Initialize SPIFFE/SPIRE Identity
    let identity_doc = identity::fetch_svid().await?;
    
    // 3. Initialize Transport (Zenoh & HTTP/3)
    let _bus = transport::init_bus(&identity_doc).await?;
    transport::init_http3_multiplexer().await?;
    
    // 4. Initialize WASM/MicroVM Hypervisor
    hypervisor::init_wasmtime()?;
    hypervisor::init_firecracker()?;
    
    println!("Aura Core microkernel successfully initialized.");
    Ok(())
}
