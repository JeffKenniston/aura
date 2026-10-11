## 2026-10-09T23:03:21Z
You are spec_miner_survey_3, a teamwork_preview_spec_miner subagent.
Working directory: /home/jeff/aura/.agents/teamwork/spec_miner_survey_3/
Parent: Project Orchestrator (orchestrator_2)

Mandatory: Read /home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md first.

Scope: Phase M3 (Cognition: Cognitive Processing & Observability, Model Router, Token Economics, Interactions API).

Your task:
1. Thoroughly investigate authoritative requirements sources:
   - /home/jeff/aura/.agents/teamwork/ORIGINAL_REQUEST.md
   - /home/jeff/projects/aura/aura-os/docs/ROADMAP.md (Phase 3 items)
   - /home/jeff/projects/aura/aura-os/docs/KNOWLEDGE.md and RESEARCH.md
   - /home/jeff/aura/docs/architecture/adrs/ADR-002-Identity-Routing-Repo.md
   - /home/jeff/aura/.agents/plugins/aura-dev-kit/skills/ (aura-cog-domain, aura-rte-domain, aura-architecture/references/05-gemini-interactions-routing.md, 07-guardrails-token-economics.md)
   - Codebase comments and docstrings in aura-sdk/aura_sdk/cognition/, aura-sdk/aura_sdk/routing/, aura-core/src/transport/rate_limiter.rs, aura-core/src/transport/webhook.rs, aura-core/src/config/, aura-sdk/tests/
2. Enumerate EVERY requirement in scope:
   - Identify all requirement IDs: COG-001 through COG-014, RTE-001 through RTE-009, GAP-001 through GAP-008 (specifically context saturation and cache mitigation).
   - For each requirement, determine:
     * Requirement ID & full description
     * Source document (ADR, skill, roadmap, code comment)
     * Implementation Status: Complete, Partial, or Missing
     * Exact code citations (file paths and line numbers) where implemented or tested
     * Specific next steps required to reach "Complete" if partial or missing
3. Output: Write your complete findings to /home/jeff/aura/.agents/teamwork/spec_miner_survey_3/survey_report.md
4. Deliver your handoff.md and notify the parent via send_message.
