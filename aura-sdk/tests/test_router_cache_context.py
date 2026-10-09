"""
Tests for ModelRouter integration with Semantic Cache and Context Saturation.
Verifies COG-013 and GAP-006 integration in the ModelRouter decision matrix.
"""

from unittest.mock import AsyncMock, MagicMock

import pytest

from aura_sdk.cognition.cache import CacheEntry
from aura_sdk.cognition.context import ContextManager
from aura_sdk.routing.router import (
    ModelRouter,
    Tier,
    route_cognitive_demand_async,
)


def test_router_semantic_cache_hit_sync():
    """Verifies router returns cached completion with 0 tokens when cache hit occurs."""
    mock_cache = MagicMock()
    cached_entry = CacheEntry(
        prompt="Explain photosynthesis",
        prompt_hash="abc123hash",
        completion="Photosynthesis is the process...",
        similarity=1.0,
        tokens_saved=300,
    )
    mock_cache.lookup_sync.return_value = cached_entry

    router = ModelRouter(cache_client=mock_cache)
    res = router.route("Explain photosynthesis")

    assert res.get("cached") is True
    assert res.get("cached_response") == "Photosynthesis is the process..."
    assert res.get("model") == "gemini-3.8-flash"

    # Verify decision logged
    assert len(router.decision_logs) == 1
    log = router.decision_logs[0]
    assert "Semantic cache hit" in log.reason
    assert log.metadata.get("cached") is True


def test_router_semantic_cache_miss_proceeds_to_standard_routing():
    """Verifies router proceeds to normal routing when cache misses."""
    mock_cache = MagicMock()
    mock_cache.lookup_sync.return_value = None

    router = ModelRouter(cache_client=mock_cache)
    res = router.route("Plan the complete database schema")

    assert "cached" not in res
    # High cognitive demand keywords -> Pro tier
    assert res == {"model": "gemini-3.1-pro"}


def test_router_context_saturation_routes_to_flash_lite():
    """Verifies router detects 80% context saturation and routes to Flash-Lite for summarization."""
    router = ModelRouter()

    # Under 80%: 750k / 1M = 75% -> Normal Flash routing
    res_under = router.route(
        "Routine worker task",
        context_tokens=750_000,
        max_context_tokens=1_000_000,
    )
    assert res_under == {"model": "gemini-3.8-flash"}
    assert "requires_summarization" not in res_under

    # At or above 80%: 820k / 1M = 82% -> Routes to Flash-Lite for summarization
    res_saturated = router.route(
        "Routine worker task",
        context_tokens=820_000,
        max_context_tokens=1_000_000,
    )
    assert res_saturated.get("requires_summarization") is True
    assert res_saturated.get("context_saturation") is True
    assert res_saturated.get("model") == "gemini-3.5-flash-lite"

    # Verify audit log
    log = router.decision_logs[-1]
    assert log.tier == Tier.FLASH_LITE
    assert "Context window saturation" in log.reason


def test_router_with_context_manager_instance():
    """Verifies router integrates directly with a ContextManager instance."""
    cm = ContextManager(max_tokens=1_000_000, saturation_threshold=0.80)
    router = ModelRouter(context_manager=cm)

    # Context is not saturated yet
    cm.record_tokens(500_000)
    res = router.route("Standard instruction")
    assert res == {"model": "gemini-3.8-flash"}

    # Push to saturation
    cm.record_tokens(900_000)
    res_sat = router.route("Standard instruction")
    assert res_sat.get("requires_summarization") is True
    assert res_sat.get("model") == "gemini-3.5-flash-lite"


@pytest.mark.asyncio
async def test_router_route_async_with_remote_cache():
    """Verifies route_async awaits remote Zenoh cache before finalizing routing."""
    mock_cache = MagicMock()
    mock_cache.lookup = AsyncMock()

    cached_entry = CacheEntry(
        prompt="Async cached prompt",
        prompt_hash="xyz789",
        completion="Async cached output",
    )
    mock_cache.lookup.return_value = cached_entry

    router = ModelRouter()
    res = await router.route_async("Async cached prompt", cache_client=mock_cache)

    assert res.get("cached") is True
    assert res.get("cached_response") == "Async cached output"
    mock_cache.lookup.assert_awaited_once_with("Async cached prompt", tenant_id="default")


@pytest.mark.asyncio
async def test_route_cognitive_demand_async_helper():
    """Verifies the global route_cognitive_demand_async helper function."""
    res = await route_cognitive_demand_async(
        "Plan the distributed storage architecture",
        strict_options=False,
    )
    assert res == {"model": "gemini-3.1-pro"}
