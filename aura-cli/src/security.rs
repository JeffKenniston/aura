use spiffe::workload_api::client::WorkloadApiClient;
// use spiffe_rustls_tokio::{SpiffeClientConfig, SpiffeServerConfig}; // We may need to find the correct types
use cedar_policy::{Authorizer, Context, Entities, EntityUid, Request};
use std::str::FromStr;

pub async fn get_spiffe_config() -> Result<(), Box<dyn std::error::Error>> {
    // Stub for SPIFFE/SPIRE mTLS Attestation dynamic X.509 SVID acquisition.
    Ok(())
}

pub enum ExecutionMode {
    Strict,
    RequestReview,
    ProceedInSandbox,
    AlwaysProceed,
}

pub struct CedarEngine {
    authorizer: Authorizer,
}

impl CedarEngine {
    pub fn new() -> Self {
        Self {
            authorizer: Authorizer::new(),
        }
    }

    pub fn evaluate_execution_mode(&self, principal: &str, action: &str, resource: &str) -> ExecutionMode {
        let principal_uid = EntityUid::from_str(principal).unwrap_or_else(|_| EntityUid::from_str("User::\"unknown\"").unwrap());
        let action_uid = EntityUid::from_str(action).unwrap_or_else(|_| EntityUid::from_str("Action::\"unknown\"").unwrap());
        let resource_uid = EntityUid::from_str(resource).unwrap_or_else(|_| EntityUid::from_str("Resource::\"unknown\"").unwrap());

        let _request = Request::new(
            principal_uid,
            action_uid,
            resource_uid,
            Context::empty(),
            None,
        ).unwrap();

        let _entities = Entities::empty();
        
        ExecutionMode::RequestReview
    }
}
