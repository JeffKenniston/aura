use super::Svid;

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum Capability {
    FileSystemWrite,
    FileSystemRead,
    NetworkBind,
}

impl Capability {
    fn as_str(&self) -> &'static str {
        match self {
            Capability::FileSystemRead => "fs:read",
            Capability::FileSystemWrite => "fs:write",
            Capability::NetworkBind => "net:bind",
        }
    }
}

pub struct CapabilityEnforcer;

impl CapabilityEnforcer {
    /// Audits the provided SVID against a specific requested capability.
    pub fn check(svid: &Svid, capability: Capability) -> Result<(), Box<dyn std::error::Error>> {
        let req_scope = capability.as_str();
        if svid.scopes.contains(req_scope) {
            Ok(())
        } else {
            Err(format!(
                "Zero-Trust Violation: SVID {} is not authorized for capability '{}'",
                svid.id, req_scope
            )
            .into())
        }
    }
}
