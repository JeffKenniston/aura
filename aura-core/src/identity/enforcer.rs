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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_capability_as_str() {
        assert_eq!(Capability::FileSystemRead.as_str(), "fs:read");
        assert_eq!(Capability::FileSystemWrite.as_str(), "fs:write");
        assert_eq!(Capability::NetworkBind.as_str(), "net:bind");
    }

    #[test]
    fn test_enforcer_check_authorized() {
        let mut scopes = HashSet::new();
        scopes.insert("fs:read".to_string());

        let svid = Svid {
            id: "spiffe://example.org/test".to_string(),
            scopes,
        };

        let result = CapabilityEnforcer::check(&svid, Capability::FileSystemRead);
        assert!(result.is_ok());
    }

    #[test]
    fn test_enforcer_check_unauthorized() {
        let mut scopes = HashSet::new();
        scopes.insert("fs:read".to_string());

        let svid = Svid {
            id: "spiffe://example.org/test".to_string(),
            scopes,
        };

        let result = CapabilityEnforcer::check(&svid, Capability::FileSystemWrite);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert_eq!(
            err_msg,
            "Zero-Trust Violation: SVID spiffe://example.org/test is not authorized for capability 'fs:write'"
        );
    }
}
