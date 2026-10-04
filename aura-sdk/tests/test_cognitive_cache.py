import asyncio
from unittest.mock import MagicMock, AsyncMock, patch
import pytest

from google.genai import types
from aura_sdk.agent import Agent

@pytest.mark.asyncio
@patch("aura_sdk.agent.genai.Client")
async def test_compression_logic(mock_client_cls):
    agent = Agent(name="WorkerAgent", svid="spiffe://test/agent/worker")
    
    # Mock transport connection
    agent.transport.connect = AsyncMock()
    agent.transport.session = MagicMock()
    await agent.boot()
    
    # Manually trigger compression flag (simulating zenoh bus reception)
    agent.compression_required = True
    agent.previous_interaction_id = "session_123"
    
    mock_client = MagicMock()
    agent.client = mock_client
    
    # Mock the interactions.create response stream for summarization
    async def mock_summary_stream():
        class MockDelta:
            def __init__(self):
                self.type = "text"
                self.text = "Mocked Summary"
        class MockEvent:
            def __init__(self):
                self.event_type = "step.delta"
                self.delta = MockDelta()
        yield MockEvent()
    
    # Mock the interactions.create response stream for actual task
    async def mock_task_stream():
        class MockUsage:
            def __init__(self):
                self.total_tokens = 900000
        class MockInteraction:
            def __init__(self):
                self.id = "session_new"
                self.usage = MockUsage()
        class MockEvent:
            def __init__(self):
                self.event_type = "interaction.completed"
                # Add usage struct
                self.interaction = MockInteraction()
        yield MockEvent()

    mock_client.aio.interactions.create = AsyncMock()
    mock_client.aio.interactions.create.side_effect = [
        mock_summary_stream(),
        mock_task_stream()
    ]
    
    agent.transport.publish = MagicMock()

    await agent.execute_task("Next command")
    
    # Verify client was called with summary instructions
    calls = mock_client.aio.interactions.create.call_args_list
    assert len(calls) == 2
    
    summary_call = calls[0]
    assert summary_call.kwargs["input"] == "System Context Compression: Summarize the entire conversation history, architectural decisions, and current state into a dense context document."
    assert summary_call.kwargs["model"] == "gemini-3.5-flash-lite"
    assert summary_call.kwargs["previous_interaction_id"] == "session_123"
    
    task_call = calls[1]
    assert "Previous context summary: Mocked Summary" in task_call.kwargs["input"]
    assert "Next instruction: Next command" in task_call.kwargs["input"]
    assert "previous_interaction_id" not in task_call.kwargs  # Should be None after compression
    
    # Verify that total_tokens were published
    agent.transport.publish.assert_called_with(
        "tasks/metrics/tokens",
        '{"agent": "WorkerAgent", "session_id": "", "total_tokens": 900000}'
    )

