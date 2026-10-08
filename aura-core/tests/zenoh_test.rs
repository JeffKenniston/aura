use aura_core::identity::Svid;
use aura_core::transport::zenoh_bus::ZenohBus;
use std::collections::HashSet;

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_zenoh_initialization() {
    let svid = Svid {
        id: "spiffe://aura.local/test".to_string(),
        scopes: HashSet::new(),
    };
    let bus_result = ZenohBus::new(&svid).await;
    assert!(bus_result.is_ok(), "Failed to initialize ZenohBus");
}
