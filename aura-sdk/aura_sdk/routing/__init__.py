"""Routing subpackage exposing ModelRouter, Tier, and routing functions."""

from .router import (
    DEFAULT_TIER_MODELS,
    BudgetExceededError,
    ModelRouter,
    ProTierBlockedError,
    RoutingDecision,
    TenantBudget,
    Tier,
    route_cognitive_demand,
)

__all__ = [
    "DEFAULT_TIER_MODELS",
    "BudgetExceededError",
    "ModelRouter",
    "ProTierBlockedError",
    "RoutingDecision",
    "TenantBudget",
    "Tier",
    "route_cognitive_demand",
]
