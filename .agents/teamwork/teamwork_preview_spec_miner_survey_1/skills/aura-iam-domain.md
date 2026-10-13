# Local copy of aura-iam-domain
Source: /home/jeff/aura/.agents/plugins/aura-dev-kit/skills/aura-iam-domain/SKILL.md

# IAM Domain Rules
- **Identity**: Every workload gets a SPIFFE SVID. Short-lived TTL.
- **Format**: `spiffe://<trust-domain>/<tenant>/<kind>/<name>/<instance>`
- **Authorization**: Every capability call requires an SVID with authorized scopes.
- **Escalation**: Scopes never broadened at runtime.
- **Auditing**: Log all decisions (ID, scope, resource, result).
