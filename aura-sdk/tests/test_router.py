"""
Tests for ModelRouter and route_cognitive_demand.
Verifies RTE-001 through RTE-009.
"""

import pytest

from aura_sdk.routing.router import (
    DEFAULT_TIER_MODELS,
    BudgetExceededError,
    ModelRouter,
    ProTierBlockedError,
    Tier,
    route_cognitive_demand,
)


# Legacy test compatibility
def test_route_deep_research():
    assert route_cognitive_demand("synthesize a market analysis") == {"agent": "deep-research-preview"}


def test_route_pro():
    assert route_cognitive_demand("plan the system architecture") == {"model": "gemini-3.1-pro"}
    assert route_cognitive_demand("a" * 2001) == {"model": "gemini-3.1-pro"}


def test_route_flash_lite():
    assert route_cognitive_demand("please validate this log file") == {"model": "gemini-3.5-flash-lite"}


def test_route_flash():
    assert route_cognitive_demand("write a simple python script to do this") == {"model": "gemini-3.8-flash"}


# RTE-001: Support four tiers
def test_rte_001_four_tiers_configuration():
    assert len(Tier) == 4
    assert Tier.PRO in DEFAULT_TIER_MODELS
    assert Tier.FLASH in DEFAULT_TIER_MODELS
    assert Tier.FLASH_LITE in DEFAULT_TIER_MODELS
    assert Tier.DEEP_RESEARCH in DEFAULT_TIER_MODELS


# RTE-002: Default tier is Flash (gemini-3.8-flash)
def test_rte_002_default_tier_is_flash():
    router = ModelRouter()
    decision = router.route("Do something routine")
    assert decision == {"model": "gemini-3.8-flash"}


# RTE-003: Cognitive demand, latency budget and cost budget
def test_rte_003_decision_table_heuristics():
    router = ModelRouter()

    # Latency constraint -> Flash-Lite
    res_lat = router.route("Generic task", latency_budget_ms=500)
    assert res_lat == {"model": "gemini-3.5-flash-lite"}

    # Cost constraint -> Flash-Lite
    res_cost = router.route("Generic task", cost_budget=0.0005)
    assert res_cost == {"model": "gemini-3.5-flash-lite"}

    # High cognitive demand -> Pro
    res_cog = router.route("Multi-step deep reasoning and architectural planning")
    assert res_cog == {"model": "gemini-3.1-pro"}


# RTE-004: Deep Research shall ONLY be called through an explicit agent option
def test_rte_004_deep_research_explicit_option_only():
    router = ModelRouter()

    # Without explicit option, prompt with research keywords routes to default/pro, NEVER Deep Research
    res_no_opt = router.route("Conduct a comprehensive literature review and market analysis")
    assert "agent" not in res_no_opt
    assert res_no_opt["model"] in ("gemini-3.8-flash", "gemini-3.1-pro")

    # With explicit agent option, routes to Deep Research
    res_with_opt = router.route(
        "Conduct a comprehensive literature review",
        agent_options={"deep_research": True},
    )
    assert res_with_opt == {"agent": "deep-research-preview"}


# RTE-005: AST blast radius escalation
def test_rte_005_blast_radius_escalation():
    router = ModelRouter(blast_radius_threshold=3)

    # Low blast radius (under threshold) -> Flash
    res_low = router.route("Refactor helper function", ast_blast_radius=2)
    assert res_low == {"model": "gemini-3.8-flash"}

    # High blast radius (>= threshold) -> Escalated to Pro
    res_high = router.route("Refactor helper function", ast_blast_radius=4)
    assert res_high == {"model": "gemini-3.1-pro"}


# RTE-006: Hot config reload
def test_rte_006_hot_config_reload():
    router = ModelRouter()
    assert router.tier_models[Tier.PRO] == "gemini-3.1-pro"

    # Reload config with new model version
    new_cfg = {
        "models": {
            "pro": "gemini-3.1-pro-preview-03",
            "flash": "gemini-3.8-flash-v2",
        },
        "blast_radius_threshold": 5,
    }
    router.reload_config(new_cfg)

    assert router.tier_models[Tier.PRO] == "gemini-3.1-pro-preview-03"
    assert router.tier_models[Tier.FLASH] == "gemini-3.8-flash-v2"
    assert router.blast_radius_threshold == 5


# RTE-007: Log every routing decision and its reason
def test_rte_007_decision_audit_logging():
    router = ModelRouter()
    router.route("Perform complex system architecture", tenant_id="tenant-audit-1")

    assert len(router.decision_logs) == 1
    log_entry = router.decision_logs[0]
    assert log_entry.tenant_id == "tenant-audit-1"
    assert log_entry.tier == Tier.PRO
    assert log_entry.model == "gemini-3.1-pro"
    assert "architectural" in log_entry.reason.lower()


# RTE-008: Per-tenant token and cost budgets with soft and hard limits
def test_rte_008_budget_enforcement():
    router = ModelRouter()
    router.set_tenant_budget("tenant-budget-test", hard_cost=10.0, soft_cost=8.0)

    # Within budget
    router.record_usage("tenant-budget-test", cost=5.0)
    res = router.route("Normal task", tenant_id="tenant-budget-test")
    assert res == {"model": "gemini-3.8-flash"}

    # Cross hard limit
    router.record_usage("tenant-budget-test", cost=6.0)  # total 11.0 > 10.0
    with pytest.raises(BudgetExceededError, match="exceeded hard budget limits"):
        router.route("Next task", tenant_id="tenant-budget-test")


# RTE-009: 90% Cost Budget Webhook & Pro tier blocking
def test_rte_009_billing_webhook_and_pro_tier_blocking():
    webhook_calls = []

    def mock_billing_webhook(tenant_id: str, used_cost: float, hard_limit: float) -> bool:
        webhook_calls.append((tenant_id, used_cost, hard_limit))
        return False  # Deny budget expansion

    router = ModelRouter(billing_webhook=mock_billing_webhook)
    router.set_tenant_budget("tenant-webhook-test", hard_cost=100.0, soft_cost=80.0)

    # Spend 92% of budget
    router.record_usage("tenant-webhook-test", cost=92.0)

    # Webhook triggers when routing
    # Pro tier task should be blocked because webhook denied
    with pytest.raises(ProTierBlockedError, match="Pro tier is blocked by billing webhook"):
        router.route("Complex multi-step architectural plan", tenant_id="tenant-webhook-test")

    assert len(webhook_calls) == 1
    assert webhook_calls[0][0] == "tenant-webhook-test"
    assert webhook_calls[0][1] == 92.0

    # Flash / Flash-Lite tasks are still allowed when under hard limit
    res_flash = router.route("Simple script edit", tenant_id="tenant-webhook-test")
    assert res_flash == {"model": "gemini-3.8-flash"}
