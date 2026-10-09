"""
Multi-Tiered Model Router for Aura SDK.
Implements RTE-001 - RTE-009, COG-013 (Semantic Cache integration),
and GAP-006 (Context Saturation checks).
"""

from __future__ import annotations

import json
import logging
import time
from collections.abc import Callable
from dataclasses import dataclass, field
from enum import Enum
from typing import Any

logger = logging.getLogger("aura_sdk.routing")


class Tier(str, Enum):
    """The four supported model tiers (RTE-001)."""

    PRO = "pro"
    FLASH = "flash"
    FLASH_LITE = "flash-lite"
    DEEP_RESEARCH = "deep-research"


DEFAULT_TIER_MODELS: dict[Tier, str] = {
    Tier.PRO: "gemini-3.1-pro",
    Tier.FLASH: "gemini-3.8-flash",  # Default (RTE-002)
    Tier.FLASH_LITE: "gemini-3.5-flash-lite",
    Tier.DEEP_RESEARCH: "deep-research-preview",
}


class BudgetExceededError(Exception):
    """Raised when a tenant exceeds hard token or cost budgets (RTE-008)."""


class ProTierBlockedError(Exception):
    """Raised when Pro tier is blocked due to 90% budget webhook denial (RTE-009)."""


@dataclass
class TenantBudget:
    """Per-tenant token and cost budgets with soft and hard limits (RTE-008)."""

    tenant_id: str
    hard_cost_limit: float = 100.0
    soft_cost_limit: float = 80.0
    hard_token_limit: int = 10_000_000
    soft_token_limit: int = 8_000_000
    used_cost: float = 0.0
    used_tokens: int = 0
    pro_blocked: bool = False

    def is_hard_limit_exceeded(self) -> bool:
        return self.used_cost >= self.hard_cost_limit or self.used_tokens >= self.hard_token_limit

    def is_soft_limit_exceeded(self) -> bool:
        return self.used_cost >= self.soft_cost_limit or self.used_tokens >= self.soft_token_limit

    def is_at_or_above_90_percent_cost(self) -> bool:
        return self.used_cost >= (0.90 * self.hard_cost_limit)


@dataclass
class RoutingDecision:
    """Audit log record for every routing decision (RTE-007)."""

    tier: Tier
    model: str
    reason: str
    tenant_id: str
    timestamp: float = field(default_factory=time.time)
    metadata: dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> dict[str, Any]:
        return {
            "tier": self.tier.value,
            "model": self.model,
            "reason": self.reason,
            "tenant_id": self.tenant_id,
            "timestamp": self.timestamp,
            "metadata": self.metadata,
        }


class ModelRouter:
    """
    Multi-tiered model router managing tier selection, semantic cache lookups,
    context saturation mitigation, budget limits, AST blast radius escalation,
    and decision audits (RTE-001 - RTE-009, COG-013, GAP-006).
    """

    def __init__(
        self,
        config: dict[str, Any] | None = None,
        blast_radius_threshold: int = 3,
        billing_webhook: Callable[[str, float, float], bool] | None = None,
        cache_client: Any | None = None,
        context_manager: Any | None = None,
        summarization_tier: Tier = Tier.FLASH_LITE,
    ):
        self.blast_radius_threshold = blast_radius_threshold
        self.billing_webhook = billing_webhook
        self.cache_client = cache_client
        self.context_manager = context_manager
        self.summarization_tier = summarization_tier
        self.tier_models: dict[Tier, str] = dict(DEFAULT_TIER_MODELS)
        self.tenant_budgets: dict[str, TenantBudget] = {}
        self.decision_logs: list[RoutingDecision] = []

        if config:
            self.reload_config(config)

    def reload_config(self, config: dict[str, Any] | str) -> None:
        """
        Hot config reload for tier model IDs and parameters without code change (RTE-006).
        """
        if isinstance(config, str):
            config_dict = json.loads(config)
        else:
            config_dict = config

        # Reload model mappings
        models_cfg = config_dict.get("models", config_dict)
        for tier_key in Tier:
            if tier_key.value in models_cfg:
                self.tier_models[tier_key] = models_cfg[tier_key.value]
            elif tier_key.name.lower() in models_cfg:
                self.tier_models[tier_key] = models_cfg[tier_key.name.lower()]

        if "blast_radius_threshold" in config_dict:
            self.blast_radius_threshold = int(config_dict["blast_radius_threshold"])

        logger.info(
            f"Hot config reloaded: {self.tier_models}, threshold={self.blast_radius_threshold}"
        )

    def set_tenant_budget(
        self,
        tenant_id: str,
        hard_cost: float = 100.0,
        soft_cost: float = 80.0,
        hard_tokens: int = 10_000_000,
        soft_tokens: int = 8_000_000,
    ) -> TenantBudget:
        budget = TenantBudget(
            tenant_id=tenant_id,
            hard_cost_limit=hard_cost,
            soft_cost_limit=soft_cost,
            hard_token_limit=hard_tokens,
            soft_token_limit=soft_tokens,
        )
        self.tenant_budgets[tenant_id] = budget
        return budget

    def record_usage(self, tenant_id: str, tokens: int = 0, cost: float = 0.0) -> None:
        """Updates tenant token and cost spend."""
        if tenant_id not in self.tenant_budgets:
            self.set_tenant_budget(tenant_id)
        budget = self.tenant_budgets[tenant_id]
        budget.used_tokens += tokens
        budget.used_cost += cost

    def _check_and_enforce_budget(self, tenant_id: str, target_tier: Tier) -> None:
        """
        Enforces per-tenant soft and hard limits (RTE-008) and
        90% synchronous webhook verification (RTE-009).
        """
        if tenant_id not in self.tenant_budgets:
            return

        budget = self.tenant_budgets[tenant_id]

        # RTE-008: Hard limit blocks all new interactions
        if budget.is_hard_limit_exceeded():
            raise BudgetExceededError(
                f"Tenant '{tenant_id}' has exceeded hard budget limits "
                f"(cost: ${budget.used_cost:.2f}/${budget.hard_cost_limit:.2f}, "
                f"tokens: {budget.used_tokens}/{budget.hard_token_limit}). New interactions blocked."
            )

        if budget.is_soft_limit_exceeded():
            logger.warning(
                f"Tenant '{tenant_id}' has crossed soft budget limit "
                f"(cost: ${budget.used_cost:.2f}/${budget.soft_cost_limit:.2f})."
            )

        # RTE-009: At 90% hard cost budget, emit synchronous webhook
        if budget.is_at_or_above_90_percent_cost():
            if self.billing_webhook:
                approved = self.billing_webhook(tenant_id, budget.used_cost, budget.hard_cost_limit)
                if not approved:
                    budget.pro_blocked = True
                    logger.warning(
                        f"Billing webhook denied budget expansion for tenant '{tenant_id}'. Pro tier blocked."
                    )

            if budget.pro_blocked and target_tier == Tier.PRO:
                raise ProTierBlockedError(
                    f"Tenant '{tenant_id}' reached 90% hard cost limit and Pro tier is blocked by billing webhook."
                )

    def route(
        self,
        prompt: str,
        agent_options: dict[str, Any] | None = None,
        ast_blast_radius: int | None = None,
        latency_budget_ms: float | None = None,
        cost_budget: float | None = None,
        tenant_id: str = "default",
        strict_options: bool = True,
        cache_client: Any | None = None,
        context_tokens: int | None = None,
        max_context_tokens: int | None = None,
        context_manager: Any | None = None,
        check_cache: bool = True,
    ) -> dict[str, Any]:
        """
        Evaluates semantic cache, context window saturation, cognitive demand,
        latency, cost budgets, and AST blast radius to select model tier
        (RTE-001 - RTE-009, COG-013, GAP-006).
        """
        options = agent_options or {}

        # 1. Semantic Cache Pre-Check (COG-013)
        active_cache = cache_client or self.cache_client
        if check_cache and active_cache is not None and hasattr(active_cache, "lookup_sync"):
            cached_entry = active_cache.lookup_sync(prompt, tenant_id=tenant_id)
            if cached_entry is not None:
                selected_tier = Tier.FLASH
                model_id = self.tier_models[selected_tier]
                reason = "Semantic cache hit (0 tokens consumed, <50ms)"
                completion_text = getattr(cached_entry, "completion", "")

                decision = RoutingDecision(
                    tier=selected_tier,
                    model=model_id,
                    reason=reason,
                    tenant_id=tenant_id,
                    metadata={
                        "cached": True,
                        "prompt_hash": getattr(cached_entry, "prompt_hash", ""),
                    },
                )
                self.decision_logs.append(decision)
                logger.info(f"Routed [{tenant_id}] to cache hit: {reason}")
                return {
                    "model": model_id,
                    "cached": True,
                    "cached_response": completion_text,
                    "cached_entry": cached_entry,
                }

        # 2. Context Saturation Mitigation Check (GAP-006)
        active_cm = context_manager or self.context_manager
        is_saturated = False
        if options.get("context_saturated") or options.get("requires_summarization"):
            is_saturated = True
        elif (
            context_tokens is not None and max_context_tokens is not None and max_context_tokens > 0
        ):
            is_saturated = (context_tokens / float(max_context_tokens)) >= 0.80
        elif active_cm is not None and hasattr(active_cm, "is_saturated"):
            is_saturated = active_cm.is_saturated(
                tokens=context_tokens, max_tokens=max_context_tokens
            )

        if is_saturated:
            selected_tier = self.summarization_tier
            model_id = self.tier_models[selected_tier]
            reason = (
                "Context window saturation >= 80% detected; routed to Flash-Lite for summarization"
            )
            self._check_and_enforce_budget(tenant_id, selected_tier)

            decision = RoutingDecision(
                tier=selected_tier,
                model=model_id,
                reason=reason,
                tenant_id=tenant_id,
                metadata={
                    "context_saturation": True,
                    "requires_summarization": True,
                    "context_tokens": context_tokens,
                    "max_context_tokens": max_context_tokens,
                },
            )
            self.decision_logs.append(decision)
            logger.info(f"Routed [{tenant_id}] to {selected_tier.value} ({model_id}): {reason}")
            return {
                "model": model_id,
                "requires_summarization": True,
                "context_saturation": True,
            }

        # 3. Standard Model Tier Selection Heuristics (RTE-001 - RTE-005)
        prompt_lower = prompt.lower()
        selected_tier = Tier.FLASH
        reason = "Default orchestration tier (Flash)"

        # 3a. RTE-004: Deep Research shall only be called through an explicit agent option.
        deep_research_opt = options.get("deep_research") or (
            options.get("agent_option") == "deep-research"
        )
        if deep_research_opt:
            selected_tier = Tier.DEEP_RESEARCH
            reason = "Explicit agent option 'deep_research' enabled"
        elif not strict_options:
            # Fallback only for non-strict legacy compatibility
            deep_research_keywords = [
                "research",
                "collect data",
                "literature review",
                "competitive landscaping",
                "market analysis",
                "synthesize",
            ]
            if any(kw in prompt_lower for kw in deep_research_keywords):
                selected_tier = Tier.DEEP_RESEARCH
                reason = "Legacy keyword match for Deep Research (non-strict)"

        # If not deep research, check AST Blast Radius escalation (RTE-005)
        if selected_tier != Tier.DEEP_RESEARCH:
            if ast_blast_radius is not None and ast_blast_radius >= self.blast_radius_threshold:
                selected_tier = Tier.PRO
                reason = f"AST blast radius ({ast_blast_radius}) >= threshold ({self.blast_radius_threshold}); escalated to Pro"

            # Check Cognitive Demand / Planning (RTE-003)
            elif (
                any(
                    kw in prompt_lower
                    for kw in [
                        "plan",
                        "architecture",
                        "complex",
                        "design specification",
                        "deep reasoning",
                        "multi-step",
                    ]
                )
                or len(prompt) > 2000
            ):
                selected_tier = Tier.PRO
                reason = "High cognitive demand / architectural planning / large context length"

            # Check Latency and Cost Budgets (RTE-003)
            elif (latency_budget_ms is not None and latency_budget_ms < 1000) or (
                cost_budget is not None and cost_budget < 0.001
            ):
                selected_tier = Tier.FLASH_LITE
                reason = f"Budget constraints (latency_ms={latency_budget_ms}, cost={cost_budget}); routed to Flash-Lite"

            elif any(
                kw in prompt_lower
                for kw in [
                    "validate",
                    "classify",
                    "parse",
                    "format",
                    "check",
                    "log analysis",
                    "cost-sensitive",
                ]
            ):
                selected_tier = Tier.FLASH_LITE
                reason = "Lightweight validation or parsing task; routed to Flash-Lite"

        # Check and enforce budgets (RTE-008, RTE-009)
        self._check_and_enforce_budget(tenant_id, selected_tier)

        model_id = self.tier_models[selected_tier]

        # RTE-007: Log every routing decision and its reason
        decision = RoutingDecision(
            tier=selected_tier,
            model=model_id,
            reason=reason,
            tenant_id=tenant_id,
            metadata={
                "ast_blast_radius": ast_blast_radius,
                "latency_budget_ms": latency_budget_ms,
                "cost_budget": cost_budget,
                "prompt_length": len(prompt),
                "options": options,
            },
        )
        self.decision_logs.append(decision)
        logger.info(f"Routed [{tenant_id}] to {selected_tier.value} ({model_id}): {reason}")

        if selected_tier == Tier.DEEP_RESEARCH:
            return {"agent": model_id}
        return {"model": model_id}

    async def route_async(
        self,
        prompt: str,
        agent_options: dict[str, Any] | None = None,
        ast_blast_radius: int | None = None,
        latency_budget_ms: float | None = None,
        cost_budget: float | None = None,
        tenant_id: str = "default",
        strict_options: bool = True,
        cache_client: Any | None = None,
        context_tokens: int | None = None,
        max_context_tokens: int | None = None,
        context_manager: Any | None = None,
        check_cache: bool = True,
    ) -> dict[str, Any]:
        """
        Asynchronous variant of route() that awaits remote semantic cache lookups over Zenoh.
        """
        active_cache = cache_client or self.cache_client
        if check_cache and active_cache is not None:
            cached_entry = None
            if hasattr(active_cache, "lookup"):
                cached_entry = await active_cache.lookup(prompt, tenant_id=tenant_id)
            elif hasattr(active_cache, "lookup_sync"):
                cached_entry = active_cache.lookup_sync(prompt, tenant_id=tenant_id)

            if cached_entry is not None:
                selected_tier = Tier.FLASH
                model_id = self.tier_models[selected_tier]
                reason = "Semantic cache hit (0 tokens consumed, <50ms)"
                completion_text = getattr(cached_entry, "completion", "")

                decision = RoutingDecision(
                    tier=selected_tier,
                    model=model_id,
                    reason=reason,
                    tenant_id=tenant_id,
                    metadata={
                        "cached": True,
                        "prompt_hash": getattr(cached_entry, "prompt_hash", ""),
                    },
                )
                self.decision_logs.append(decision)
                logger.info(f"Routed [{tenant_id}] to cache hit: {reason}")
                return {
                    "model": model_id,
                    "cached": True,
                    "cached_response": completion_text,
                    "cached_entry": cached_entry,
                }

        # Proceed with context saturation and standard routing
        return self.route(
            prompt=prompt,
            agent_options=agent_options,
            ast_blast_radius=ast_blast_radius,
            latency_budget_ms=latency_budget_ms,
            cost_budget=cost_budget,
            tenant_id=tenant_id,
            strict_options=strict_options,
            cache_client=active_cache,
            context_tokens=context_tokens,
            max_context_tokens=max_context_tokens,
            context_manager=context_manager,
            check_cache=False,  # Already evaluated
        )


# Default global router instance
_GLOBAL_ROUTER = ModelRouter()


def route_cognitive_demand(
    prompt: str,
    agent_options: dict[str, Any] | None = None,
    ast_blast_radius: int | None = None,
    latency_budget_ms: float | None = None,
    cost_budget: float | None = None,
    tenant_id: str = "default",
    strict_options: bool = False,
    cache_client: Any | None = None,
    context_tokens: int | None = None,
    max_context_tokens: int | None = None,
    context_manager: Any | None = None,
    check_cache: bool = True,
) -> dict[str, Any]:
    """
    Evaluates cognitive demand and returns routing configuration.
    Maintains compatibility with legacy callers while supporting all RTE-001 - RTE-009,
    COG-013, and GAP-006 parameters.
    """
    return _GLOBAL_ROUTER.route(
        prompt=prompt,
        agent_options=agent_options,
        ast_blast_radius=ast_blast_radius,
        latency_budget_ms=latency_budget_ms,
        cost_budget=cost_budget,
        tenant_id=tenant_id,
        strict_options=strict_options,
        cache_client=cache_client,
        context_tokens=context_tokens,
        max_context_tokens=max_context_tokens,
        context_manager=context_manager,
        check_cache=check_cache,
    )


async def route_cognitive_demand_async(
    prompt: str,
    agent_options: dict[str, Any] | None = None,
    ast_blast_radius: int | None = None,
    latency_budget_ms: float | None = None,
    cost_budget: float | None = None,
    tenant_id: str = "default",
    strict_options: bool = False,
    cache_client: Any | None = None,
    context_tokens: int | None = None,
    max_context_tokens: int | None = None,
    context_manager: Any | None = None,
    check_cache: bool = True,
) -> dict[str, Any]:
    """
    Asynchronous version of route_cognitive_demand that awaits Zenoh semantic cache lookups.
    """
    return await _GLOBAL_ROUTER.route_async(
        prompt=prompt,
        agent_options=agent_options,
        ast_blast_radius=ast_blast_radius,
        latency_budget_ms=latency_budget_ms,
        cost_budget=cost_budget,
        tenant_id=tenant_id,
        strict_options=strict_options,
        cache_client=cache_client,
        context_tokens=context_tokens,
        max_context_tokens=max_context_tokens,
        context_manager=context_manager,
        check_cache=check_cache,
    )
