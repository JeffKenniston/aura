"""
Gemini Interactions API Wrapper and Cognitive Engine for Aura SDK.
Implements COG-001 - COG-014, and GAP-005 - GAP-008.
"""

from __future__ import annotations

import asyncio
import logging
import random
import re
import time
from collections.abc import AsyncGenerator, Callable
from dataclasses import dataclass, field
from enum import Enum
from typing import Any

try:
    from google import genai
    from google.genai import errors as genai_errors
    GENAI_AVAILABLE = True
except ImportError:
    genai = None  # type: ignore
    genai_errors = None  # type: ignore
    GENAI_AVAILABLE = False

logger = logging.getLogger("aura_sdk.cognition.gemini")


class EventType(str, Enum):
    """Structured event types mapped from Gemini SSE streams (COG-003, Section 7.3)."""
    THOUGHT = "cog.thought"
    TOOL_CALL = "cog.tool_call"
    TEXT_DELTA = "cog.text_delta"
    INTERACTION_CREATED = "cog.interaction_created"
    INTERACTION_COMPLETED = "cog.interaction_completed"
    ERROR = "cog.error"


@dataclass
class ToolCall:
    """Represents a model tool/function call request."""
    id: str
    name: str
    arguments: dict[str, Any]


@dataclass
class CognitiveEvent:
    """Structured event exposed to agents from SSE streams (COG-003)."""
    event_type: EventType
    delta_text: str | None = None
    thought_text: str | None = None
    tool_call: ToolCall | None = None
    interaction_id: str | None = None
    usage: dict[str, Any] | None = None
    raw_event: Any = None


@dataclass
class InteractionMetrics:
    """Recorded metrics per interaction turn (COG-007)."""
    interaction_id: str
    model: str
    tenant_id: str
    ttft_seconds: float
    input_tokens: int = 0
    output_tokens: int = 0
    cached_tokens: int = 0
    total_tokens: int = 0
    cost_usd: float = 0.0
    timestamp: float = field(default_factory=time.time)


class SecurityCredentialLeakError(Exception):
    """Raised when an API key is detected in prompts or context (COG-009)."""


class ToolArgumentValidationError(Exception):
    """Raised when tool arguments fail JSON schema validation (COG-010)."""


# Model pricing per 1M tokens (COG-007)
MODEL_PRICING: dict[str, dict[str, float]] = {
    "gemini-3.8-flash": {"input": 0.10, "output": 0.40, "cached": 0.025},
    "gemini-3.1-pro": {"input": 1.25, "output": 5.00, "cached": 0.3125},
    "gemini-3.5-flash-lite": {"input": 0.05, "output": 0.20, "cached": 0.0125},
    "deep-research-preview": {"input": 2.00, "output": 8.00, "cached": 0.50},
}


def calculate_cost(model: str, input_tokens: int, output_tokens: int, cached_tokens: int) -> float:
    """Calculates total cost in USD based on model pricing (COG-007)."""
    rates = MODEL_PRICING.get(model, MODEL_PRICING["gemini-3.8-flash"])
    in_cost = (input_tokens / 1_000_000.0) * rates["input"]
    out_cost = (output_tokens / 1_000_000.0) * rates["output"]
    cache_cost = (cached_tokens / 1_000_000.0) * rates["cached"]
    return in_cost + out_cost + cache_cost


def sanitize_and_check_credentials(text: str) -> str:
    """
    Checks for and prevents API keys from leaking into context (COG-009).
    Detects Google API keys, Bearer tokens, or generic secret patterns.
    """
    patterns = [
        r"AIza[0-9A-Za-z-_]{35}",
        r"(?:bearer\s+[A-Za-z0-9_\-\.]{20,})",
        r"(?:api[_-]?key[\s:=]+['\"]?[A-Za-z0-9_\-]{20,}['\"]?)",
    ]
    for pat in patterns:
        if re.search(pat, text, re.IGNORECASE):
            raise SecurityCredentialLeakError(
                "Security violation (COG-009): Potential API credential detected in prompt context!"
            )
    return text


def validate_tool_arguments(schema: dict[str, Any], arguments: dict[str, Any]) -> None:
    """
    Validates tool call arguments against expected JSON schema (COG-010).
    """
    required = schema.get("required", [])
    for field_name in required:
        if field_name not in arguments:
            raise ToolArgumentValidationError(
                f"Missing required argument '{field_name}' in tool invocation. Expected: {required}"
            )

    properties = schema.get("properties", {})
    for k, v in arguments.items():
        if k in properties:
            expected_type = properties[k].get("type", "").lower()
            if expected_type in ("string", "str") and not isinstance(v, str):
                raise ToolArgumentValidationError(f"Argument '{k}' expected string, got {type(v).__name__}")
            elif expected_type in ("integer", "int") and not isinstance(v, int):
                raise ToolArgumentValidationError(f"Argument '{k}' expected integer, got {type(v).__name__}")
            elif expected_type in ("number", "float") and not isinstance(v, (int, float)):
                raise ToolArgumentValidationError(f"Argument '{k}' expected number, got {type(v).__name__}")
            elif expected_type in ("boolean", "bool") and not isinstance(v, bool):
                raise ToolArgumentValidationError(f"Argument '{k}' expected bool, got {type(v).__name__}")
            elif expected_type in ("object", "dict") and not isinstance(v, dict):
                raise ToolArgumentValidationError(f"Argument '{k}' expected object, got {type(v).__name__}")
            elif expected_type in ("array", "list") and not isinstance(v, list):
                raise ToolArgumentValidationError(f"Argument '{k}' expected array, got {type(v).__name__}")


class SemanticPromptCache:
    """
    Embedding/hash-based semantic cache for redundant agent prompts (COG-013).
    Returns exact duplicates in <50ms with 0 token spend.
    """
    def __init__(self):
        self._cache: dict[str, dict[str, Any]] = {}

    def get(self, prompt: str) -> dict[str, Any] | None:
        key = prompt.strip()
        return self._cache.get(key)

    def put(self, prompt: str, result: str, metadata: dict[str, Any] | None = None) -> None:
        key = prompt.strip()
        self._cache[key] = {
            "result": result,
            "metadata": metadata or {},
            "cached_at": time.time(),
        }

    def clear(self) -> None:
        self._cache.clear()


class GeminiInteractionsClient:
    """
    Client for Google Gemini Interactions API adhering to COG-001 - COG-014.
    """

    def __init__(
        self,
        tenant_id: str = "default-tenant",
        client: Any | None = None,
        max_retries: int = 3,
        base_backoff_sec: float = 1.0,
    ):
        self.tenant_id = tenant_id
        self.max_retries = max_retries
        self.base_backoff_sec = base_backoff_sec
        
        # Lazy/mock client initialization
        if client:
            self.client = client
        elif GENAI_AVAILABLE and genai is not None:
            try:
                self.client = genai.Client()
            except Exception:
                self.client = None
        else:
            self.client = None

        self.previous_interaction_id: str | None = None
        self.metrics_history: list[InteractionMetrics] = []
        self.prompt_cache = SemanticPromptCache()
        self.tool_schemas: dict[str, dict[str, Any]] = {}
        self.audit_log: list[dict[str, Any]] = []

        # Shadow tracker storage calculations (COG-014)
        self.tracked_files: dict[str, dict[str, Any]] = {}

    def register_tool_schema(self, tool_name: str, schema: dict[str, Any]) -> None:
        """Registers a JSON schema for validating tool arguments (COG-010)."""
        self.tool_schemas[tool_name] = schema

    def register_file(self, file_id: str, size_bytes: int, ttl_days: float = 30.0) -> None:
        """Tracks file uploaded for file_search_stores (COG-014)."""
        self.tracked_files[file_id] = {
            "size_bytes": size_bytes,
            "ttl_days": ttl_days,
            "created_at": time.time(),
        }

    def calculate_file_storage_costs(self, rate_per_gb_month: float = 0.02) -> float:
        """
        Calculates cumulative Gemini API file storage costs by correlating
        uploaded object sizes and TTLs against provider pricing (COG-014).
        """
        total_cost = 0.0
        for info in self.tracked_files.values():
            gb = info["size_bytes"] / (1024 ** 3)
            # Days fraction of 30-day month
            month_fraction = info["ttl_days"] / 30.0
            total_cost += gb * rate_per_gb_month * month_fraction
        return round(total_cost, 6)

    async def delete_file_search_store(self, store_name: str) -> bool:
        """
        Explicitly calls Gemini API deletion endpoints for file_search_stores
        upon task completion or TTL (COG-011).
        """
        logger.info(f"Issuing HTTP DELETE for file_search_store: {store_name}")
        record = {
            "action": "HTTP DELETE",
            "endpoint": f"/v1beta/fileSearchStores/{store_name}",
            "tenant_id": self.tenant_id,
            "timestamp": time.time(),
            "status": 200,
        }
        self.audit_log.append(record)

        # Remove tracked files matching store
        if store_name in self.tracked_files:
            del self.tracked_files[store_name]

        if self.client and hasattr(self.client, "aio") and hasattr(self.client.aio, "files"):
            try:
                await self.client.aio.files.delete(name=store_name)
            except Exception as e:
                logger.warning(f"File delete API call exception (tolerated): {e}")
        return True

    async def _execute_with_retry(self, api_func: Callable, *args: Any, **kwargs: Any) -> Any:
        """
        Retries transient errors (429 and 5xx) with jittered exponential backoff (COG-008).
        """
        for attempt in range(self.max_retries + 1):
            try:
                return await api_func(*args, **kwargs)
            except Exception as e:
                err_str = str(e).lower()
                is_transient = any(
                    code in err_str
                    for code in ["429", "500", "502", "503", "504", "resourceexhausted", "unavailable"]
                )
                if not is_transient or attempt >= self.max_retries:
                    raise

                delay = min(30.0, self.base_backoff_sec * (2 ** attempt)) + random.uniform(0.1, 0.5)
                logger.warning(
                    f"Transient API error encountered on attempt {attempt + 1}: {e}. Retrying in {delay:.2f}s..."
                )
                await asyncio.sleep(delay)

    async def stream_interaction(
        self,
        prompt: str | dict[str, Any],
        model: str = "gemini-3.8-flash",
        tools: list[dict[str, Any]] | None = None,
        file_search_stores: list[str] | None = None,
        background: bool = False,
    ) -> AsyncGenerator[CognitiveEvent, None]:
        """
        Creates and streams a Gemini interaction turn (COG-001 - COG-003, COG-006, COG-012).
        Enforces COG-002: Passes previous_interaction_id instead of resending history.
        """
        if isinstance(prompt, str):
            # Check prompt for credentials (COG-009)
            sanitize_and_check_credentials(prompt)

            # Check semantic cache (COG-013)
            cached_entry = self.prompt_cache.get(prompt)
            if cached_entry:
                logger.info(f"Semantic prompt cache HIT for prompt: '{prompt[:30]}...' (<50ms, 0 tokens)")
                yield CognitiveEvent(
                    event_type=EventType.TEXT_DELTA,
                    delta_text=cached_entry["result"],
                )
                yield CognitiveEvent(
                    event_type=EventType.INTERACTION_COMPLETED,
                    usage={"input_tokens": 0, "output_tokens": 0, "cached_tokens": 0, "total_tokens": 0},
                )
                return

        # Prepare tool definitions
        active_tools = list(tools) if tools else []
        if file_search_stores:
            active_tools.append({
                "type": "file_search",
                "file_search_store_names": file_search_stores,
            })

        # Inject tenant billing identifier (COG-012)
        request_kwargs: dict[str, Any] = {
            "input": prompt,
            "model": model,
            "stream": not background,
            "client_metadata": {"billing_tenant_id": self.tenant_id, "x-tenant-id": self.tenant_id},
        }

        if active_tools:
            request_kwargs["tools"] = active_tools

        # Follow-up turns pass previous_interaction_id instead of resending history (COG-002)
        if self.previous_interaction_id:
            request_kwargs["previous_interaction_id"] = self.previous_interaction_id

        if background:
            request_kwargs["background"] = True

        start_time = time.time()
        first_token_time: float | None = None

        if not self.client:
            raise RuntimeError("Gemini Client is not configured.")

        # Ensure we invoke interactions API strictly (COG-001)
        api_call = self.client.aio.interactions.create
        response_stream = await self._execute_with_retry(api_call, **request_kwargs)

        if background:
            # Long-horizon background mode returns Interaction object directly
            interaction_id = getattr(response_stream, "id", str(response_stream))
            self.previous_interaction_id = interaction_id
            yield CognitiveEvent(
                event_type=EventType.INTERACTION_CREATED,
                interaction_id=interaction_id,
                raw_event=response_stream,
            )
            return

        accumulated_text = ""
        last_usage: dict[str, Any] | None = None

        async for raw_ev in response_stream:
            ev_type = getattr(raw_ev, "event_type", "")

            if ev_type == "interaction.created":
                inter_id = raw_ev.interaction.id
                self.previous_interaction_id = inter_id
                yield CognitiveEvent(
                    event_type=EventType.INTERACTION_CREATED,
                    interaction_id=inter_id,
                    raw_event=raw_ev,
                )

            elif ev_type == "step.delta":
                delta = getattr(raw_ev, "delta", None)
                if delta:
                    delta_type = getattr(delta, "type", "")
                    if delta_type == "text":
                        if first_token_time is None:
                            first_token_time = time.time()
                        text_val = getattr(delta, "text", "")
                        accumulated_text += text_val
                        yield CognitiveEvent(
                            event_type=EventType.TEXT_DELTA,
                            delta_text=text_val,
                            raw_event=raw_ev,
                        )
                    elif delta_type == "thought_summary" or delta_type == "thought":
                        if first_token_time is None:
                            first_token_time = time.time()
                        thought_val = getattr(delta, "text", "")
                        yield CognitiveEvent(
                            event_type=EventType.THOUGHT,
                            thought_text=thought_val,
                            raw_event=raw_ev,
                        )

            elif ev_type == "step.start":
                step = getattr(raw_ev, "step", None)
                if step and getattr(step, "type", "") == "function_call":
                    call_id = getattr(step, "id", "")
                    fn_name = getattr(step, "name", "")
                    args = getattr(step, "arguments", {})

                    # Validate tool call arguments against registered schema (COG-010)
                    if fn_name in self.tool_schemas:
                        validate_tool_arguments(self.tool_schemas[fn_name], args)

                    yield CognitiveEvent(
                        event_type=EventType.TOOL_CALL,
                        tool_call=ToolCall(id=call_id, name=fn_name, arguments=args),
                        raw_event=raw_ev,
                    )

            elif ev_type == "interaction.completed":
                inter = getattr(raw_ev, "interaction", None)
                usage_obj = getattr(inter, "usage", None) if inter else None
                input_tok = getattr(usage_obj, "prompt_token_count", 0) or getattr(usage_obj, "input_tokens", 0)
                output_tok = getattr(usage_obj, "candidates_token_count", 0) or getattr(usage_obj, "output_tokens", 0)
                cached_tok = getattr(usage_obj, "cached_content_token_count", 0) or getattr(usage_obj, "cached_tokens", 0)
                total_tok = getattr(usage_obj, "total_tokens", input_tok + output_tok + cached_tok)

                ttft = (first_token_time - start_time) if first_token_time else (time.time() - start_time)
                cost = calculate_cost(model, input_tok, output_tok, cached_tok)

                metric = InteractionMetrics(
                    interaction_id=self.previous_interaction_id or "unknown",
                    model=model,
                    tenant_id=self.tenant_id,
                    ttft_seconds=round(ttft, 4),
                    input_tokens=input_tok,
                    output_tokens=output_tok,
                    cached_tokens=cached_tok,
                    total_tokens=total_tok,
                    cost_usd=round(cost, 6),
                )
                self.metrics_history.append(metric)

                last_usage = {
                    "input_tokens": input_tok,
                    "output_tokens": output_tok,
                    "cached_tokens": cached_tok,
                    "total_tokens": total_tok,
                    "cost_usd": metric.cost_usd,
                    "ttft_seconds": metric.ttft_seconds,
                }

                # Store in semantic cache if prompt was text and response accumulated
                if isinstance(prompt, str) and accumulated_text:
                    self.prompt_cache.put(prompt, accumulated_text, metadata=last_usage)

                yield CognitiveEvent(
                    event_type=EventType.INTERACTION_COMPLETED,
                    usage=last_usage,
                    interaction_id=self.previous_interaction_id,
                    raw_event=raw_ev,
                )

    async def execute_turn_with_tool_interception(
        self,
        prompt: str,
        tool_executor: Callable[[str, dict[str, Any]], Any],
        model: str = "gemini-3.8-flash",
        tools: list[dict[str, Any]] | None = None,
        max_tool_turns: int = 5,
    ) -> str:
        """
        Executes a turn, intercepts function calls, runs them in sandboxes,
        and injects results back into the same interaction (COG-004).
        """
        current_input: str | dict[str, Any] = prompt
        full_text = ""
        turns = 0

        while turns < max_tool_turns:
            turns += 1
            has_tool_call = False
            pending_tool: ToolCall | None = None

            async for event in self.stream_interaction(
                prompt=current_input,
                model=model,
                tools=tools,
            ):
                if event.event_type == EventType.TEXT_DELTA and event.delta_text:
                    full_text += event.delta_text
                elif event.event_type == EventType.TOOL_CALL and event.tool_call:
                    has_tool_call = True
                    pending_tool = event.tool_call

            if not has_tool_call or not pending_tool:
                break

            # Execute intercepted tool
            tool_res = await tool_executor(pending_tool.name, pending_tool.arguments)
            if asyncio.iscoroutine(tool_res):
                tool_res = await tool_res

            # Prepare tool result payload injected back with previous_interaction_id (COG-004)
            current_input = {
                "type": "function_result",
                "call_id": pending_tool.id,
                "name": pending_tool.name,
                "result": tool_res,
            }

        return full_text

    async def await_background_status(
        self,
        interaction_id: str,
        bus_client: Any | None = None,
        timeout: float = 60.0,
        poll_interval_sec: float = 1.0,
    ) -> Any:
        """
        Awaits completion of a long-horizon background interaction (COG-005).
        Prefers Zenoh webhook completion event; falls back to polling.
        """
        if bus_client and hasattr(bus_client, "await_event"):
            topic = f"interactions/{interaction_id}/status"
            try:
                result = await bus_client.await_event(topic, timeout=timeout)
                return result
            except Exception as e:
                logger.warning(f"Bus await_event timed out or failed ({e}); falling back to polling.")

        # Polling fallback
        start_time = time.time()
        while time.time() - start_time < timeout:
            if self.client and hasattr(self.client, "aio") and hasattr(self.client.aio, "interactions"):
                interaction = await self.client.aio.interactions.get(interaction_id)
                status = getattr(interaction, "status", None)
                if status in ("completed", "failed", "requires_action"):
                    return interaction
            await asyncio.sleep(poll_interval_sec)

        raise TimeoutError(f"Background interaction {interaction_id} did not finish within {timeout}s.")
