use cedar_policy::{Authorizer, Context, Decision, Entities, PolicySet, Request};
use std::str::FromStr;

#[derive(Debug, PartialEq)]
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
    pub fn new() -> Result<Self, String> {
        let policy_str = r#"
            // Default rule: Request Review for tool executions
            permit(
                principal,
                action == Action::"exec",
                resource
            ) when {
                context.tier == "request-review"
            };
        "#;
        
        let policies = PolicySet::from_str(policy_str).map_err(|e| e.to_string())?;
        
        Ok(Self {
            authorizer: Authorizer::new(),
            policies,
        })
    }

    pub fn evaluate_tool_request(&self, tool_name: &str, requested_tier: PermissionTier) -> Result<Decision, String> {
        let principal = r#"User::"aura-cli""#.parse().unwrap();
        let action = r#"Action::"exec""#.parse().unwrap();
        let resource = format!(r#"Tool::"{}""#, tool_name).parse().unwrap();
        
        let context_json = match requested_tier {
            PermissionTier::Strict => serde_json::json!({"tier": "strict"}),
            PermissionTier::RequestReview => serde_json::json!({"tier": "request-review"}),
            PermissionTier::ProceedInSandbox => serde_json::json!({"tier": "proceed-in-sandbox"}),
            PermissionTier::AlwaysProceed => serde_json::json!({"tier": "always-proceed"}),
        };
        
        let context = Context::from_json_value(context_json, None).map_err(|e| e.to_string())?;
        
        let request = Request::new(Some(principal), Some(action), Some(resource), context, None)
            .map_err(|e| e.to_string())?;
            
        let entities = Entities::empty();
        let response = self.authorizer.is_authorized(&request, &self.policies, &entities);
        
        Ok(response.decision())
    }
}
