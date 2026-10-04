use std::sync::Arc;
use zenoh::prelude::r#async::*;
use zenoh::Session;
use crate::identity::Svid;

pub struct ZenohBus {
    session: Arc<Session>,
    pub prefix: String,
}

impl ZenohBus {
    pub async fn new(svid: &Svid) -> Result<Self, Box<dyn std::error::Error>> {
        let config = zenoh::config::Config::default();
        let session = zenoh::open(config).res_async().await.map_err(|e| e.to_string())?;

        // Enforce the SPIFFE SVID topic prefixing boundary
        // E.g., spiffe://aura.local/host becomes spiffe/aura.local/host
        let sanitized_id = svid.id.replace("://", "/").replace("//", "/");
        let prefix = format!("aura/workspace/{}", sanitized_id);

        Ok(Self {
            session: Arc::new(session),
            prefix,
        })
    }

    /// Publishes a payload to a scoped sub-topic.
    pub async fn publish<T: Into<zenoh::value::Value>>(&self, sub_topic: &str, payload: T) -> Result<(), Box<dyn std::error::Error>> {
        let full_topic = format!("{}/{}", self.prefix, sub_topic);
        self.session.put(full_topic, payload).res_async().await.map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Subscribes to a scoped sub-topic.
    pub async fn subscribe(&self, sub_topic: &str) -> Result<zenoh::subscriber::Subscriber<'_, flume::Receiver<zenoh::sample::Sample>>, Box<dyn std::error::Error>> {
        let full_topic = format!("{}/{}", self.prefix, sub_topic);
        let subscriber = self.session.declare_subscriber(full_topic).res_async().await.map_err(|e| e.to_string())?;
        Ok(subscriber)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_zenoh_bus_pub_sub() -> Result<(), Box<dyn std::error::Error>> {
        let svid = Svid { id: "spiffe://test".to_string(), scopes: std::collections::HashSet::new() };
        let bus = ZenohBus::new(&svid).await?;

        // 1. Subscribe to the scoped topic
        let subscriber = bus.subscribe("events").await?;

        // 2. Publish a message to the same topic
        let test_payload = "zero-copy message";
        bus.publish("events", test_payload).await?;

        // 3. Receive the message
        let sample = subscriber.recv_async().await?;
        
        // 4. Verify contents and identity scoping
        let binding = sample.payload.contiguous();
        let received_str = String::from_utf8_lossy(&binding);
        assert_eq!(received_str, test_payload);
        assert!(sample.key_expr.as_str().starts_with("aura/workspace/spiffe/test/"));

        Ok(())
    }
}
