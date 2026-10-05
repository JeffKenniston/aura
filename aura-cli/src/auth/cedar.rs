use cedar_policy::{Authorizer, Context, Decision, Entities, EntityUid, PolicySet, Request};
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionTier {
    Strict,
    RequestReview,
    ProceedInSandbox,
    AlwaysProceed,
}

pub struct CedarAuthorizer {
    authorizer: Authorizer,
    policies: PolicySet,
}

impl CedarAuthorizer {
    pub fn new(policy_str: &str) -> Result<Self, cedar_policy::ParseErrors> {
        let policies = PolicySet::from_str(policy_str)?;
        let authorizer = Authorizer::new();
        Ok(Self { authorizer, policies })
    }

    pub fn evaluate(
        &self,
        principal: &str,
        action: &str,
        resource: &str,
        entities: &Entities,
    ) -> Result<PermissionTier, String> {
        let principal_uid = EntityUid::from_str(principal).map_err(|e| e.to_string())?;
        let action_uid = EntityUid::from_str(action).map_err(|e| e.to_string())?;
        let resource_uid = EntityUid::from_str(resource).map_err(|e| e.to_string())?;

        let request = Request::new(
            Some(principal_uid),
            Some(action_uid),
            Some(resource_uid),
            Context::empty(),
            None,
        ).map_err(|e| e.to_string())?;

        let answer = self.authorizer.is_authorized(&request, &self.policies, entities);

        match answer.decision() {
            Decision::Allow => {
                // In a real implementation we would inspect the annotations or specific policy IDs
                // to distinguish between RequestReview, ProceedInSandbox, and AlwaysProceed.
                Ok(PermissionTier::RequestReview)
            }
            Decision::Deny => Ok(PermissionTier::Strict),
        }
    }
}
