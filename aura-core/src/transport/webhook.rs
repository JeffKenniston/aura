use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use zenoh::prelude::r#async::*;

#[derive(Deserialize, Serialize, Debug)]
struct WebhookPayload {
    interaction_id: String,
    status: String,
}

pub async fn start_webhook_listener(
    bus: Arc<crate::transport::zenoh_bus::ZenohBus>,
) -> Result<(), Box<dyn std::error::Error>> {
    let addr: SocketAddr = "127.0.0.1:8080".parse()?;
    let listener = TcpListener::bind(addr).await?;
    println!("Webhook listener bound to {}", addr);

    tokio::spawn(async move {
        loop {
            if let Ok((mut stream, _)) = listener.accept().await {
                let bus_clone = bus.clone();
                tokio::spawn(async move {
                    let mut buffer = [0; 4096];
                    if let Ok(bytes_read) = stream.read(&mut buffer).await {
                        let request_str = String::from_utf8_lossy(&buffer[..bytes_read]);

                        // Extremely simple HTTP POST parser for JSON payload
                        if let Some(body_start) = request_str.find("\r\n\r\n") {
                            let body = &request_str[(body_start + 4)..];
                            // Parse JSON body
                            if let Ok(payload) = serde_json::from_str::<WebhookPayload>(
                                body.trim_matches('\0').trim(),
                            ) {
                                println!(
                                    "Received webhook for interaction: {}, status: {}",
                                    payload.interaction_id, payload.status
                                );

                                // Publish to Zenoh
                                // The Python client subscribes to: prefix + /interactions/{interaction_id}/status
                                let sub_topic =
                                    format!("interactions/{}/status", payload.interaction_id);
                                match serde_json::to_string(&payload) {
                                    Ok(json_payload) => {
                                        let _ = bus_clone.publish(&sub_topic, json_payload).await;
                                    }
                                    Err(e) => {
                                        eprintln!("Failed to serialize webhook payload: {}", e);
                                    }
                                }
                            }
                        }

                        // Respond with 200 OK
                        let response =
                            b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                        let _ = stream.write_all(response).await;
                    }
                });
            }
        }
    });

    Ok(())
}
