pub mod http3;

pub use http3::{start_quic_listener, Http3Transport};

/// Initializes native HTTP/3 QUIC transport multiplexer (NET-001 - NET-007).
pub async fn init_http3_multiplexer() -> Result<(), Box<dyn std::error::Error>> {
    println!("Initializing native HTTP/3 QUIC transport via tokio-quiche...");
    http3::start_quic_listener().await?;
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
                // Handle port binding collisions during concurrent test execution
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
