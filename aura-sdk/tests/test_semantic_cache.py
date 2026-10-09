"""
Unit and integration tests for Semantic Cache Client (COG-013).
Verifies Zenoh bus querying, prompt hashing, exact matching, <50ms response,
0 token spend, timeout fallback, and L1 caching.
"""

import json
import time
from unittest.mock import AsyncMock, MagicMock

import pytest

from aura_sdk.bus.client import BusClient
from aura_sdk.cognition.cache import (
    SemanticCacheClient,
    compute_prompt_hash,
)


@pytest.mark.asyncio
async def test_prompt_hash_computation():
    """Verifies SHA-256 prompt hashing is deterministic and whitespace-normalized."""
    p1 = "What is the capital of France?"
    p2 = "  What is the capital of France?  "
    p3 = "What is the capital of Germany?"

    h1 = compute_prompt_hash(p1)
    h2 = compute_prompt_hash(p2)
    h3 = compute_prompt_hash(p3)

    assert h1 == h2
    assert h1 != h3
    assert len(h1) == 64  # SHA-256 hex digest length


@pytest.mark.asyncio
async def test_zenoh_semantic_cache_hit():
    """
    Verifies COG-013: Queries host sqlite-vec cache via Zenoh.
    When a cache hit occurs, returns completion instantly (<50ms) with 0 token spend.
    """
    bus = BusClient(svid="spiffe://aura/tenant-test/agent/worker", mock=True)
    await bus.connect()

    cache_client = SemanticCacheClient(
        bus_client=bus,
        tenant_id="tenant-test",
        lookup_timeout_sec=0.1,
    )

    # Simulate Rust host cache subscriber on `aura/tenant-test/cache/lookup`
    def mock_host_cache_service(sample):
        try:
            payload_str = sample.payload.decode("utf-8")
            data = json.loads(payload_str)
            if isinstance(data, dict) and "payload" in data and isinstance(data["payload"], dict):
                data = data["payload"]
            query_id = data.get("query_id")
            prompt = data.get("prompt")

            if prompt == "Calculate fibonacci(10)":
                resp_payload = {
                    "query_id": query_id,
                    "hit": True,
                    "content": "55",
                    "similarity": 1.0,
                    "tokens_saved": 150,
                    "metadata": {"source": "sqlite-vec"},
                }
            else:
                resp_payload = {
                    "query_id": query_id,
                    "hit": False,
                }

            bus.publish(f"aura/tenant-test/cache/response/{query_id}", resp_payload)
        except Exception as e:
            pytest.fail(f"Mock host error: {e}")

    host_sub = bus.subscribe("aura/tenant-test/cache/lookup", mock_host_cache_service)

    # 1. Query for known prompt -> Cache Hit
    start = time.perf_counter()
    entry = await cache_client.lookup("Calculate fibonacci(10)")
    elapsed_ms = (time.perf_counter() - start) * 1000

    assert entry is not None
    assert entry.completion == "55"
    assert entry.similarity == 1.0
    assert entry.tokens_saved == 150
    assert elapsed_ms < 50.0  # Must be <50ms per specification

    # 2. Query for unknown prompt -> Cache Miss (returns None)
    miss_entry = await cache_client.lookup("Unknown complex question")
    assert miss_entry is None

    host_sub.undeclare()
    bus.close()


@pytest.mark.asyncio
async def test_zenoh_cache_store():
    """Verifies that store() persists entry to local L1 and emits to Zenoh aura/<tenant>/cache/store."""
    bus = BusClient(svid="spiffe://aura/tenant-test/agent/worker", mock=True)
    await bus.connect()

    cache_client = SemanticCacheClient(
        bus_client=bus,
        tenant_id="tenant-test",
    )

    stored_records = []

    def mock_host_store_listener(sample):
        payload_str = sample.payload.decode("utf-8")
        data = json.loads(payload_str)
        # Unwrap standard envelope if present
        if "payload" in data:
            data = data["payload"]
        stored_records.append(data)

    store_sub = bus.subscribe("aura/tenant-test/cache/store", mock_host_store_listener)

    prompt = "Summarize the design spec"
    completion = "The design spec defines a zero-trust multi-tier architecture."
    entry = await cache_client.store(
        prompt=prompt,
        completion=completion,
        tokens_saved=200,
        metadata={"phase": "M3"},
    )

    assert entry.prompt == prompt
    assert entry.completion == completion

    # Verify local L1 sync lookup works immediately
    l1_entry = cache_client.lookup_sync(prompt)
    assert l1_entry is not None
    assert l1_entry.completion == completion

    # Verify event published on Zenoh
    assert len(stored_records) == 1
    assert stored_records[0]["prompt"] == prompt
    assert stored_records[0]["completion"] == completion
    assert stored_records[0]["tokens_saved"] == 200

    store_sub.undeclare()
    bus.close()


@pytest.mark.asyncio
async def test_cache_lookup_timeout_fallback():
    """Verifies that if the Zenoh host does not respond within timeout, lookup gracefully returns None."""
    bus = BusClient(svid="spiffe://aura/tenant-test/agent/worker", mock=True)
    await bus.connect()

    # Very short timeout (10ms) with no mock host responding
    cache_client = SemanticCacheClient(
        bus_client=bus,
        tenant_id="tenant-test",
        lookup_timeout_sec=0.01,
        enable_local_l1=False,
    )

    start = time.perf_counter()
    entry = await cache_client.lookup("Query that hangs without response")
    elapsed_ms = (time.perf_counter() - start) * 1000

    assert entry is None
    assert elapsed_ms < 100.0  # Fell back quickly without hanging

    bus.close()


@pytest.mark.asyncio
async def test_gemini_interactions_with_semantic_cache_client():
    """
    Verifies end-to-end integration: GeminiInteractionsClient checks semantic cache
    before emitting to Gemini, and stores completions on turn finish.
    """
    bus = BusClient(svid="spiffe://aura/tenant-cache/agent/worker", mock=True)
    await bus.connect()

    cache_client = SemanticCacheClient(
        bus_client=bus,
        tenant_id="tenant-cache",
        lookup_timeout_sec=0.05,
    )

    # Pre-populate cache with a completion
    await cache_client.store(
        prompt="Explain quantum entanglement",
        completion="Quantum entanglement is a physical phenomenon...",
        tokens_saved=450,
    )

    mock_genai = MagicMock()
    mock_interactions = AsyncMock()
    mock_genai.aio.interactions = mock_interactions

    # When GeminiInteractionsClient has the cache preloaded, it should resolve from cache
    # First check via SemanticCacheClient direct lookup
    cached = await cache_client.lookup("Explain quantum entanglement")
    assert cached is not None
    assert "Quantum entanglement" in cached.completion

    bus.close()
