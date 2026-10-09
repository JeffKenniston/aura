"""
Semantic Cache Client for Aura SDK (COG-013).
Queries the host's sqlite-vec semantic cache via Zenoh before emitting to Gemini.
Returns cached completions instantly (<50ms) with 0 token spend.
"""

from __future__ import annotations

import asyncio
import hashlib
import json
import logging
import time
import uuid
from collections.abc import Callable
from dataclasses import dataclass, field
from typing import Any

logger = logging.getLogger("aura_sdk.cognition.cache")


def compute_prompt_hash(prompt: str) -> str:
    """Computes SHA-256 hash of normalized prompt for exact-match caching."""
    normalized = prompt.strip().encode("utf-8")
    return hashlib.sha256(normalized).hexdigest()


@dataclass
class CacheEntry:
    """Represents a cached prompt completion entry."""

    prompt: str
    prompt_hash: str
    completion: str
    similarity: float = 1.0
    tokens_saved: int = 0
    tenant_id: str = "default"
    metadata: dict[str, Any] = field(default_factory=dict)
    timestamp: float = field(default_factory=time.time)

    def to_dict(self) -> dict[str, Any]:
        return {
            "prompt": self.prompt,
            "prompt_hash": self.prompt_hash,
            "completion": self.completion,
            "similarity": self.similarity,
            "tokens_saved": self.tokens_saved,
            "tenant_id": self.tenant_id,
            "metadata": self.metadata,
            "timestamp": self.timestamp,
        }


@dataclass
class CacheLookupResult:
    """Result of a semantic cache lookup operation."""

    hit: bool
    entry: CacheEntry | None = None
    similarity: float = 0.0
    duration_ms: float = 0.0


class SemanticCacheClient:
    """
    Client for interacting with host-side semantic vector cache via Zenoh (COG-013).
    Supports SHA-256 exact matching, optional vector embeddings, and an in-memory
    L1 fast cache for sub-millisecond retrieval.
    """

    def __init__(
        self,
        bus_client: Any | None = None,
        tenant_id: str = "default",
        similarity_threshold: float = 0.95,
        lookup_timeout_sec: float = 0.05,  # 50ms default target per COG-013
        enable_local_l1: bool = True,
        embedding_fn: Callable[[str], list[float]] | None = None,
    ):
        self.bus_client = bus_client
        self.tenant_id = tenant_id
        self.similarity_threshold = similarity_threshold
        self.lookup_timeout_sec = lookup_timeout_sec
        self.enable_local_l1 = enable_local_l1
        self.embedding_fn = embedding_fn

        # Local L1 cache: key -> CacheEntry
        self._l1_cache: dict[str, CacheEntry] = {}
        # Pending lookup futures: query_id -> asyncio.Future
        self._pending_lookups: dict[str, asyncio.Future] = {}
        self._response_subscriber: Any = None
        self._subscribed_tenant: str | None = None

        if self.bus_client:
            self._ensure_subscriber(self.tenant_id)

    def _ensure_subscriber(self, tenant_id: str) -> None:
        """Declares a response subscriber on Zenoh for cache lookup replies."""
        if not self.bus_client:
            return
        if self._subscribed_tenant == tenant_id and self._response_subscriber is not None:
            return

        def _on_cache_response(sample: Any) -> None:
            try:
                payload_str = (
                    sample.payload.decode("utf-8")
                    if hasattr(sample.payload, "decode")
                    else str(sample.payload)
                )
                data = json.loads(payload_str)
                # Unwrap standard BusMessage envelope if present
                if isinstance(data, dict) and "payload" in data and "correlation_id" in data:
                    inner = data.get("payload", {})
                    correlation_id = data.get("correlation_id", "")
                    if isinstance(inner, dict):
                        query_id = inner.get("query_id") or correlation_id
                        resp_data = inner
                    else:
                        query_id = correlation_id
                        resp_data = {"content": inner, "hit": True}
                else:
                    query_id = data.get("query_id") or data.get("correlation_id")
                    resp_data = data

                if query_id and query_id in self._pending_lookups:
                    fut = self._pending_lookups[query_id]
                    if not fut.done():
                        loop = asyncio.get_event_loop()
                        if loop.is_running():
                            loop.call_soon_threadsafe(fut.set_result, resp_data)
                        else:
                            fut.set_result(resp_data)
            except Exception as e:
                logger.error(f"Error processing cache response from Zenoh: {e}")

        topic = f"aura/{tenant_id}/cache/response/*"
        try:
            self._response_subscriber = self.bus_client.subscribe(topic, _on_cache_response)
            self._subscribed_tenant = tenant_id
        except Exception as e:
            logger.warning(f"Could not attach Zenoh cache response subscriber: {e}")

    async def lookup(
        self,
        prompt: str,
        tenant_id: str | None = None,
        embedding: list[float] | None = None,
    ) -> CacheEntry | None:
        """
        Queries the semantic cache for a prompt before emitting to Gemini.
        Returns CacheEntry if a hit is found, or None if a miss occurs or times out.
        """
        start_time = time.time()
        target_tenant = tenant_id or self.tenant_id
        p_hash = compute_prompt_hash(prompt)
        cache_key = f"{target_tenant}:{p_hash}"

        # 1. Check local L1 cache
        if self.enable_local_l1 and cache_key in self._l1_cache:
            entry = self._l1_cache[cache_key]
            duration_ms = (time.time() - start_time) * 1000
            logger.info(
                f"Semantic cache L1 HIT for prompt hash {p_hash[:8]} in {duration_ms:.2f}ms (0 tokens)"
            )
            return entry

        # 2. Check host-side cache via Zenoh if bus client available
        if not self.bus_client:
            return None

        self._ensure_subscriber(target_tenant)

        query_id = str(uuid.uuid4())
        loop = asyncio.get_running_loop()
        future: asyncio.Future = loop.create_future()
        self._pending_lookups[query_id] = future

        embed_vector = embedding or (self.embedding_fn(prompt) if self.embedding_fn else None)
        lookup_payload = {
            "query_id": query_id,
            "prompt": prompt,
            "prompt_hash": p_hash,
            "embedding": embed_vector,
            "similarity_threshold": self.similarity_threshold,
            "tenant_id": target_tenant,
            "timestamp": time.time(),
        }

        lookup_topic = f"aura/{target_tenant}/cache/lookup"

        try:
            # Emit query over Zenoh bus
            self.bus_client.emit(lookup_topic, lookup_payload, correlation_id=query_id)
            # Await response with strict timeout (<50ms target)
            resp = await asyncio.wait_for(future, timeout=self.lookup_timeout_sec)

            duration_ms = (time.time() - start_time) * 1000

            if isinstance(resp, dict) and resp.get("hit"):
                completion = resp.get("content") or resp.get("completion") or ""
                similarity = float(resp.get("similarity") or 1.0)
                raw_tok = resp.get("tokens_saved") or resp.get("total_tokens") or 0
                tokens_saved = int(raw_tok)
                meta = resp.get("metadata", {})

                entry = CacheEntry(
                    prompt=prompt,
                    prompt_hash=p_hash,
                    completion=completion,
                    similarity=similarity,
                    tokens_saved=tokens_saved,
                    tenant_id=target_tenant,
                    metadata=meta,
                )

                # Store in L1 cache
                if self.enable_local_l1:
                    self._l1_cache[cache_key] = entry

                logger.info(
                    f"Semantic cache Zenoh HIT for hash {p_hash[:8]} in {duration_ms:.2f}ms (0 tokens)"
                )
                return entry

            logger.debug(f"Semantic cache Zenoh MISS for hash {p_hash[:8]} in {duration_ms:.2f}ms")
            return None

        except TimeoutError:
            duration_ms = (time.time() - start_time) * 1000
            logger.debug(
                f"Semantic cache lookup timed out after {duration_ms:.2f}ms (> {self.lookup_timeout_sec * 1000:.1f}ms). Falling back to cache miss."
            )
            return None
        except Exception as e:
            logger.warning(f"Semantic cache lookup error (gracefully falling back to miss): {e}")
            return None
        finally:
            self._pending_lookups.pop(query_id, None)

    def lookup_sync(
        self,
        prompt: str,
        tenant_id: str | None = None,
    ) -> CacheEntry | None:
        """
        Synchronous lookup for non-async contexts (checks local L1 cache).
        """
        target_tenant = tenant_id or self.tenant_id
        p_hash = compute_prompt_hash(prompt)
        cache_key = f"{target_tenant}:{p_hash}"
        return self._l1_cache.get(cache_key)

    async def store(
        self,
        prompt: str,
        completion: str,
        tenant_id: str | None = None,
        embedding: list[float] | None = None,
        metadata: dict[str, Any] | None = None,
        tokens_saved: int = 0,
    ) -> CacheEntry:
        """
        Stores a prompt completion in both local L1 cache and the host-side Zenoh cache.
        """
        target_tenant = tenant_id or self.tenant_id
        p_hash = compute_prompt_hash(prompt)
        cache_key = f"{target_tenant}:{p_hash}"

        entry = CacheEntry(
            prompt=prompt,
            prompt_hash=p_hash,
            completion=completion,
            similarity=1.0,
            tokens_saved=tokens_saved,
            tenant_id=target_tenant,
            metadata=metadata or {},
            timestamp=time.time(),
        )

        if self.enable_local_l1:
            self._l1_cache[cache_key] = entry

        if self.bus_client:
            store_topic = f"aura/{target_tenant}/cache/store"
            embed_vector = embedding or (self.embedding_fn(prompt) if self.embedding_fn else None)
            store_payload = {
                "prompt": prompt,
                "prompt_hash": p_hash,
                "completion": completion,
                "embedding": embed_vector,
                "tenant_id": target_tenant,
                "metadata": metadata or {},
                "tokens_saved": tokens_saved,
                "timestamp": time.time(),
            }
            try:
                self.bus_client.emit(store_topic, store_payload)
            except Exception as e:
                logger.warning(f"Failed to publish cache store event to Zenoh: {e}")

        return entry

    def store_sync(
        self,
        prompt: str,
        completion: str,
        tenant_id: str | None = None,
        metadata: dict[str, Any] | None = None,
    ) -> CacheEntry:
        """Synchronously stores entry into local L1 cache."""
        target_tenant = tenant_id or self.tenant_id
        p_hash = compute_prompt_hash(prompt)
        cache_key = f"{target_tenant}:{p_hash}"
        entry = CacheEntry(
            prompt=prompt,
            prompt_hash=p_hash,
            completion=completion,
            tenant_id=target_tenant,
            metadata=metadata or {},
        )
        self._l1_cache[cache_key] = entry
        return entry

    def clear_local(self) -> None:
        """Clears local in-memory cache."""
        self._l1_cache.clear()
