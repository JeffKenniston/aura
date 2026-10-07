pub mod http3_mux;
pub mod webhook;
pub mod zenoh_bus;

use std::sync::Arc;

pub async fn init_bus(
    svid: &crate::identity::Svid,
) -> Result<Arc<zenoh_bus::ZenohBus>, Box<dyn std::error::Error>> {
    println!(
        "Initializing Zenoh zero-copy event bus with SVID: {}",
        svid.id
    );
    let bus = Arc::new(zenoh_bus::ZenohBus::new(svid).await?);
    bus.start_task_listener().await?;
    bus.start_tools_listener().await?;
    bus.start_token_monitor().await?;

    // Start webhook listener for background interactions
    webhook::start_webhook_listener(bus.clone()).await?;

    println!("Zenoh bus initialized successfully.");
    Ok(bus)
}

pub async fn init_http3_multiplexer() -> Result<(), Box<dyn std::error::Error>> {
    println!("Initializing native HTTP/3 QUIC transport via tokio-quiche...");
    http3_mux::start_quic_listener().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::ErrorKind;

    #[tokio::test]
    async fn test_init_http3_multiplexer() {
        let result = init_http3_multiplexer().await;
        match result {
            Ok(_) => {}
            Err(e) => {
                // To avoid flaky tests when run concurrently with other tests binding to 4433
                if let Some(io_err) = e.downcast_ref::<std::io::Error>() {
                    if io_err.kind() == ErrorKind::AddrInUse {
                        println!("Address already in use, skipping test");
                        return;
                    }
                }
                panic!("init_http3_multiplexer failed: {}", e);
            }
        }
    }
}
