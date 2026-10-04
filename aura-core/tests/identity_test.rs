use aura_core::identity::enforcer::{CapabilityEnforcer, Capability};
use aura_core::identity::Svid;
use std::collections::HashSet;

#[test]
fn test_capability_enforcement() {
    let mut scopes = HashSet::new();
    scopes.insert("fs:read".to_string());
    
    let svid = Svid {
        id: "spiffe://aura.local/agent/test".to_string(),
        scopes,
    };
    
    assert!(CapabilityEnforcer::check(&svid, Capability::FileSystemRead).is_ok());
    assert!(CapabilityEnforcer::check(&svid, Capability::FileSystemWrite).is_err());
}
