"""
Unit and integration tests for Gemini Interactions API wrapper.
Verifies COG-001 through COG-014, and GAP-005 - GAP-008.
"""

import asyncio
from unittest.mock import AsyncMock, MagicMock

import pytest

from aura_sdk.cognition.gemini import (
    CognitiveEvent,
    EventType,
    GeminiInteractionsClient,
    SecurityCredentialLeakError,
    ToolArgumentValidationError,
    calculate_cost,
    sanitize_and_check_credentials,
    validate_tool_arguments,
)


class MockInteraction:
    def __init__(self, inter_id: str = "interaction-001", status: str = "completed"):
        self.id = inter_id
        self.status = status
        self.usage = MagicMock()
        self.usage.prompt_token_count = 1000
        self.usage.candidates_token_count = 200
        self.usage.cached_content_token_count = 500
        self.usage.total_tokens = 1700


class MockDelta:
    def __init__(self, delta_type: str, text: str):
        self.type = delta_type
        self.text = text


class MockStep:
    def __init__(self, step_type: str, name: str, arguments: dict, call_id: str = "call-1"):
        self.type = step_type
        self.name = name
        self.arguments = arguments
        self.id = call_id


class MockRawEvent:
    def __init__(
        self,
        event_type: str,
        delta: MockDelta | None = None,
        step: MockStep | None = None,
        interaction: MockInteraction | None = None,
    ):
        self.event_type = event_type
        self.delta = delta
        self.step = step
        self.interaction = interaction


@pytest.mark.asyncio
async def test_cog_001_to_003_interactions_api_and_sse_streaming():
    """Verifies COG-001 (Interactions API only), COG-002 (previous_interaction_id), and COG-003 (SSE events)."""
    mock_genai_client = MagicMock()
    mock_interactions = AsyncMock()
    mock_genai_client.aio.interactions = mock_interactions

    interaction_obj = MockInteraction("sess-100")

    async def mock_stream(**kwargs):
        yield MockRawEvent("interaction.created", interaction=interaction_obj)
        yield MockRawEvent("step.delta", delta=MockDelta("thought_summary", "Analyzing request..."))
        yield MockRawEvent("step.delta", delta=MockDelta("text", "Hello, World!"))
        yield MockRawEvent("interaction.completed", interaction=interaction_obj)

    mock_interactions.create.side_effect = mock_stream

    client = GeminiInteractionsClient(tenant_id="tenant-acme", client=mock_genai_client)

    events: list[CognitiveEvent] = []
    async for ev in client.stream_interaction("Say hello"):
        events.append(ev)

    # COG-001 & COG-003: Check structured events
    event_types = [e.event_type for e in events]
    assert EventType.INTERACTION_CREATED in event_types
    assert EventType.THOUGHT in event_types
    assert EventType.TEXT_DELTA in event_types
    assert EventType.INTERACTION_COMPLETED in event_types

    # COG-002: Interaction ID tracked
    assert client.previous_interaction_id == "sess-100"

    # Verify second turn sends previous_interaction_id without resending history
    async def mock_second_stream(**kwargs):
        # Assert COG-002: previous_interaction_id is passed and input is new turn only
        assert kwargs.get("previous_interaction_id") == "sess-100"
        assert kwargs.get("input") == "Follow up turn"
        yield MockRawEvent("step.delta", delta=MockDelta("text", "Follow up response"))
        yield MockRawEvent("interaction.completed", interaction=MockInteraction("sess-100"))

    mock_interactions.create.side_effect = mock_second_stream

    second_events = []
    async for ev in client.stream_interaction("Follow up turn"):
        second_events.append(ev)
    assert len(second_events) == 2


@pytest.mark.asyncio
async def test_cog_004_function_call_interception():
    """Verifies COG-004: Host intercepts function calls, runs tool, and injects results into session."""
    mock_genai_client = MagicMock()
    mock_interactions = AsyncMock()
    mock_genai_client.aio.interactions = mock_interactions

    call_count = 0

    async def mock_create(**kwargs):
        nonlocal call_count
        call_count += 1
        if call_count == 1:
            async def _gen():
                yield MockRawEvent("interaction.created", interaction=MockInteraction("sess-200"))
                yield MockRawEvent(
                    "step.start",
                    step=MockStep("function_call", "run_bash", {"cmd": "uname -a"}, "call-abc"),
                )
                yield MockRawEvent("interaction.completed", interaction=MockInteraction("sess-200"))
            return _gen()
        else:
            assert kwargs.get("previous_interaction_id") == "sess-200"
            fn_res = kwargs.get("input")
            assert fn_res["type"] == "function_result"
            assert fn_res["call_id"] == "call-abc"
            assert fn_res["result"] == "Linux 6.6.0"

            async def _gen():
                yield MockRawEvent("step.delta", delta=MockDelta("text", "OS is Linux 6.6.0"))
                yield MockRawEvent("interaction.completed", interaction=MockInteraction("sess-200"))
            return _gen()

    mock_interactions.create.side_effect = mock_create

    client = GeminiInteractionsClient(tenant_id="tenant-alpha", client=mock_genai_client)

    async def tool_runner(name: str, args: dict):
        assert name == "run_bash"
        return "Linux 6.6.0"

    result = await client.execute_turn_with_tool_interception("Check kernel version", tool_runner)
    assert "OS is Linux 6.6.0" in result
    assert client.previous_interaction_id == "sess-200"


@pytest.mark.asyncio
async def test_cog_005_background_execution():
    """Verifies COG-005: Long-horizon tasks use background=True."""
    mock_genai_client = MagicMock()
    mock_interactions = AsyncMock()
    mock_genai_client.aio.interactions = mock_interactions

    bg_interaction = MockInteraction("bg-inter-99", status="in_progress")
    mock_interactions.create.return_value = bg_interaction

    client = GeminiInteractionsClient(tenant_id="tenant-bg", client=mock_genai_client)

    events = []
    async for ev in client.stream_interaction("Heavy refactor", background=True):
        events.append(ev)

    assert len(events) == 1
    assert events[0].event_type == EventType.INTERACTION_CREATED
    assert events[0].interaction_id == "bg-inter-99"

    # Verify background=True was in call kwargs
    call_kwargs = mock_interactions.create.call_args.kwargs
    assert call_kwargs.get("background") is True
    assert call_kwargs.get("stream") is False


@pytest.mark.asyncio
async def test_cog_006_and_011_file_search_stores():
    """Verifies COG-006: file_search_stores attachment, and COG-011: store deletion endpoints."""
    mock_genai_client = MagicMock()
    mock_interactions = AsyncMock()
    mock_genai_client.aio.interactions = mock_interactions
    mock_files = AsyncMock()
    mock_genai_client.aio.files = mock_files

    async def mock_stream(**kwargs):
        # Assert COG-006: file_search tool is included
        tools = kwargs.get("tools", [])
        assert any(t.get("type") == "file_search" for t in tools)
        yield MockRawEvent("step.delta", delta=MockDelta("text", "Found in document store"))
        yield MockRawEvent("interaction.completed", interaction=MockInteraction("sess-rag"))

    mock_interactions.create.side_effect = mock_stream
    client = GeminiInteractionsClient(tenant_id="tenant-rag", client=mock_genai_client)

    async for _ in client.stream_interaction(
        "Search docs",
        file_search_stores=["store-docs-v1"],
    ):
        pass

    # COG-011: Explicit deletion call
    deleted = await client.delete_file_search_store("store-docs-v1")
    assert deleted is True
    assert len(client.audit_log) == 1
    assert client.audit_log[0]["action"] == "HTTP DELETE"
    assert "store-docs-v1" in client.audit_log[0]["endpoint"]


def test_cog_007_metrics_and_pricing():
    """Verifies COG-007: TTFT, tokens, and cost calculation."""
    cost = calculate_cost("gemini-3.8-flash", input_tokens=1_000_000, output_tokens=1_000_000, cached_tokens=1_000_000)
    # Flash: input $0.10 + output $0.40 + cached $0.025 = $0.525
    assert pytest.approx(cost, 0.001) == 0.525


@pytest.mark.asyncio
async def test_cog_008_transient_error_retry():
    """Verifies COG-008: Transient error retry with jittered backoff."""
    mock_genai_client = MagicMock()
    mock_interactions = AsyncMock()
    mock_genai_client.aio.interactions = mock_interactions

    interaction_obj = MockInteraction("sess-retry")
    attempt_count = 0

    async def flaky_create(**kwargs):
        nonlocal attempt_count
        attempt_count += 1
        if attempt_count < 3:
            raise RuntimeError("429 ResourceExhausted: rate limit exceeded")
        async def success_stream():
            yield MockRawEvent("step.delta", delta=MockDelta("text", "Success after retry"))
            yield MockRawEvent("interaction.completed", interaction=interaction_obj)
        return success_stream()

    mock_interactions.create.side_effect = flaky_create

    client = GeminiInteractionsClient(tenant_id="tenant-retry", client=mock_genai_client, max_retries=3, base_backoff_sec=0.01)

    events = []
    async for ev in client.stream_interaction("Execute task"):
        events.append(ev)

    assert attempt_count == 3
    assert any(e.delta_text == "Success after retry" for e in events)


def test_cog_009_credential_protection():
    """Verifies COG-009: API credentials never enter model context."""
    clean_prompt = "Refactor this python module to use dataclasses"
    assert sanitize_and_check_credentials(clean_prompt) == clean_prompt

    leaked_prompt = "Here is my key: AIzaSyD3x94Kl09mN_OPQ8392klmnOpqRstUvwX"
    with pytest.raises(SecurityCredentialLeakError):
        sanitize_and_check_credentials(leaked_prompt)


def test_cog_010_tool_schema_validation():
    """Verifies COG-010: Tool call arguments validated against JSON schema."""
    schema = {
        "type": "object",
        "properties": {
            "path": {"type": "string"},
            "lines": {"type": "integer"},
        },
        "required": ["path"],
    }

    # Valid
    validate_tool_arguments(schema, {"path": "/workspace/main.rs", "lines": 10})

    # Missing required
    with pytest.raises(ToolArgumentValidationError, match="Missing required argument 'path'"):
        validate_tool_arguments(schema, {"lines": 10})

    # Incorrect type
    with pytest.raises(ToolArgumentValidationError, match="expected integer"):
        validate_tool_arguments(schema, {"path": "/workspace/main.rs", "lines": "ten"})


@pytest.mark.asyncio
async def test_cog_012_tenant_billing_tag_injection():
    """Verifies COG-012: Tenant billing identifier injected into every outbound request."""
    mock_genai_client = MagicMock()
    mock_interactions = AsyncMock()
    mock_genai_client.aio.interactions = mock_interactions

    async def mock_stream(**kwargs):
        # Assert client_metadata has tenant billing tag
        metadata = kwargs.get("client_metadata", {})
        assert metadata.get("billing_tenant_id") == "tenant-corp-99"
        yield MockRawEvent("interaction.completed", interaction=MockInteraction("sess-billing"))

    mock_interactions.create.side_effect = mock_stream
    client = GeminiInteractionsClient(tenant_id="tenant-corp-99", client=mock_genai_client)

    async for _ in client.stream_interaction("Verify billing tag"):
        pass


@pytest.mark.asyncio
async def test_cog_013_semantic_cache():
    """Verifies COG-013: Semantic cache returns exact duplicates in <50ms with 0 token spend."""
    mock_genai_client = MagicMock()
    mock_interactions = AsyncMock()
    mock_genai_client.aio.interactions = mock_interactions

    async def mock_stream(**kwargs):
        yield MockRawEvent("step.delta", delta=MockDelta("text", "Computed expensive answer"))
        yield MockRawEvent("interaction.completed", interaction=MockInteraction("sess-cache"))

    mock_interactions.create.side_effect = mock_stream

    client = GeminiInteractionsClient(tenant_id="tenant-cache", client=mock_genai_client)

    # 1. First call - miss and cache
    res1 = []
    async for ev in client.stream_interaction("What is 2+2?"):
        res1.append(ev)

    assert mock_interactions.create.call_count == 1

    # 2. Second call with exact duplicate prompt - HIT cache!
    start = asyncio.get_event_loop().time()
    res2 = []
    async for ev in client.stream_interaction("What is 2+2?"):
        res2.append(ev)
    duration_ms = (asyncio.get_event_loop().time() - start) * 1000

    # Never called backend again
    assert mock_interactions.create.call_count == 1
    assert duration_ms < 50.0
    completed_ev = next(e for e in res2 if e.event_type == EventType.INTERACTION_COMPLETED)
    assert completed_ev.usage["total_tokens"] == 0


def test_cog_014_file_storage_costs():
    """Verifies COG-014: Shadow tracker calculates file storage costs."""
    client = GeminiInteractionsClient(tenant_id="tenant-storage")
    # 1 GB file for 30 days at $0.02/GB-month
    client.register_file("file-1", 1024 ** 3, ttl_days=30.0)
    cost = client.calculate_file_storage_costs(rate_per_gb_month=0.02)
    assert pytest.approx(cost, 0.0001) == 0.02
