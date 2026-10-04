pub mod zenoh_bus;
pub mod http3_mux;

pub async fn init_bus(svid: &crate::identity::Svid) -> Result<zenoh_bus::ZenohBus, Box<dyn std::error::Error>> {
    println!("Initializing Zenoh zero-copy event bus with SVID: {}", svid.id);
    let bus = zenoh_bus::ZenohBus::new(svid).await?;
    println!("Zenoh bus initialized successfully.");
    Ok(bus)
}

pub async fn init_http3_multiplexer() -> Result<(), Box<dyn std::error::Error>> {
    println!("Initializing native HTTP/3 QUIC transport via tokio-quiche...");
    http3_mux::start_quic_listener().await?;
    Ok(())
}
