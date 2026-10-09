"""
Unit and integration tests for Context Saturation Mitigation Engine (GAP-006, GAP-008).
Verifies:
1. 80% context window saturation detection.
2. Task pause / resume lifecycle.
3. Automated summarization using Flash-Lite.
4. Seeding new Interaction ID with distilled background context.
5. Enforcement of hard limit on consecutive summarizations to prevent infinite loops.
"""

import json
from unittest.mock import AsyncMock, MagicMock

import pytest

from aura_sdk.bus.client import BusClient
from aura_sdk.cognition.context import (
    ContextManager,
    InfiniteSummarizationError,
    check_context_saturation,
)


def test_saturation_threshold_detection():
    """Verifies GAP-006: 80% context saturation threshold logic."""
    max_tokens = 1_000_000

    # 79% -> Not saturated
    assert not check_context_saturation(790_000, max_tokens, 0.80)

    # 80% -> Saturated
    assert check_context_saturation(800_000, max_tokens, 0.80)

    # 85% -> Saturated
    assert check_context_saturation(850_000, max_tokens, 0.80)


def test_context_manager_token_recording_and_model_limits():
    """Verifies ContextManager tracks tokens and respects per-model limits."""
    cm = ContextManager(max_tokens=1_000_000, saturation_threshold=0.80)

    # Flash model (1M context)
    cm.set_model_context_limit("gemini-3.8-flash")
    assert cm.state.max_tokens == 1_000_000

    # Record 750k tokens -> False
    saturated = cm.record_tokens(750_000, interaction_id="sess-1")
    assert not saturated
    assert not cm.is_saturated()

    # Record 805k tokens -> True (80.5% >= 80%)
    saturated = cm.record_tokens(805_000, interaction_id="sess-1")
    assert saturated
    assert cm.is_saturated()

    # Pro model has 2M tokens limit
    cm.set_model_context_limit("gemini-3.1-pro")
    assert cm.state.max_tokens == 2_000_000
    # 805k / 2M = 40.25% -> Not saturated in Pro
    assert not cm.is_saturated()


@pytest.mark.asyncio
async def test_pause_and_resume_lifecycle_with_zenoh():
    """Verifies tasks are paused before summarization and resumed after, with Zenoh notifications."""
    bus = BusClient(svid="spiffe://aura/tenant-context/agent/worker", mock=True)
    await bus.connect()

    events = []

    def mock_pause_listener(sample):
        payload = json.loads(sample.payload.decode("utf-8"))
        events.append(("pause", payload))

    def mock_resume_listener(sample):
        payload = json.loads(sample.payload.decode("utf-8"))
        events.append(("resume", payload))

    sub_pause = bus.subscribe("aura/tenant-context/context/pause", mock_pause_listener)
    sub_resume = bus.subscribe("aura/tenant-context/context/resume", mock_resume_listener)

    cm = ContextManager(
        bus_client=bus,
        tenant_id="tenant-context",
        max_tokens=1_000_000,
        saturation_threshold=0.80,
    )

    cm.pause_task()
    assert cm.state.is_paused is True

    cm.resume_task()
    assert cm.state.is_paused is False

    assert len(events) == 2
    assert events[0][0] == "pause"
    assert events[1][0] == "resume"

    sub_pause.undeclare()
    sub_resume.undeclare()
    bus.close()


@pytest.mark.asyncio
async def test_summarization_with_flash_lite_and_new_seed_id():
    """
    Verifies GAP-006: Summarizes with Flash-Lite, seeds a new Interaction ID,
    and prepends background context to the next instruction.
    """
    cm = ContextManager(
        max_tokens=1_000_000,
        saturation_threshold=0.80,
        summarization_model="gemini-3.5-flash-lite",
    )

    # Set up mock Gemini client
    mock_client = MagicMock()
    mock_interactions = AsyncMock()
    mock_client.aio.interactions = mock_interactions

    async def mock_summary_stream():
        class MockDelta:
            def __init__(self, text):
                self.type = "text"
                self.text = text

        class MockEvent:
            def __init__(self, text):
                self.event_type = "step.delta"
                self.delta = MockDelta(text)

        yield MockEvent("Condensed summary of previous turns.")

    mock_interactions.create.return_value = mock_summary_stream()

    original_prompt = "Refactor database migration scripts"
    cm.state.current_tokens = 850_000
    cm.state.current_interaction_id = "old-session-42"

    distilled_prompt, new_seed_id = await cm.summarize_and_seed(
        client=mock_client,
        current_prompt=original_prompt,
    )

    # Verify summarization API call parameters
    create_call = mock_interactions.create.call_args
    assert create_call.kwargs["model"] == "gemini-3.5-flash-lite"
    assert create_call.kwargs["previous_interaction_id"] == "old-session-42"
    assert "System Context Compression" in create_call.kwargs["input"]

    # Verify new seed ID and formatted prompt
    assert new_seed_id.startswith("seed-")
    assert new_seed_id != "old-session-42"
    assert "Previous context summary: Condensed summary of previous turns." in distilled_prompt
    assert f"Next instruction: {original_prompt}" in distilled_prompt

    # State updated
    assert cm.state.consecutive_summarizations == 1
    assert cm.state.is_paused is False
    assert len(cm.state.summary_history) == 1


@pytest.mark.asyncio
async def test_infinite_summarization_hard_limit():
    """
    Verifies GAP-008: Hard limit on consecutive summarization loops prevents
    infinite recursive summarization.
    """
    max_loops = 3
    cm = ContextManager(
        max_tokens=1_000_000,
        saturation_threshold=0.80,
        max_consecutive_summarizations=max_loops,
    )

    mock_client = MagicMock()
    mock_interactions = AsyncMock()
    mock_client.aio.interactions = mock_interactions

    async def mock_stream():
        class MockEvent:
            event_type = "step.delta"
            delta = MagicMock(type="text", text="Summary.")

        yield MockEvent()

    mock_interactions.create.return_value = mock_stream()

    # Loop 1: OK
    await cm.summarize_and_seed(mock_client, "Task 1", interaction_id="s1")
    assert cm.state.consecutive_summarizations == 1

    # Loop 2: OK
    await cm.summarize_and_seed(mock_client, "Task 2", interaction_id="s2")
    assert cm.state.consecutive_summarizations == 2

    # Loop 3: OK
    await cm.summarize_and_seed(mock_client, "Task 3", interaction_id="s3")
    assert cm.state.consecutive_summarizations == 3

    # Loop 4: Exceeds limit -> raises InfiniteSummarizationError
    with pytest.raises(
        InfiniteSummarizationError, match="Hard limit of 3 consecutive summarizations exceeded"
    ):
        await cm.summarize_and_seed(mock_client, "Task 4", interaction_id="s4")

    # Verify reset clears the counter
    cm.reset_consecutive_loops()
    assert cm.state.consecutive_summarizations == 0

    # Next attempt succeeds
    await cm.summarize_and_seed(mock_client, "Task 5", interaction_id="s5")
    assert cm.state.consecutive_summarizations == 1
