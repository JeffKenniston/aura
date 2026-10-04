import json
from unittest.mock import AsyncMock, MagicMock, patch

import pytest

from aura_sdk.agent import Agent


class FakeEvent:
    def __init__(self, event_type, step=None, delta=None, interaction=None):
        self.event_type = event_type
        self.step = step
        self.delta = delta
        self.interaction = interaction

class FakeStep:
    def __init__(self, name, type="function_call", arguments=None):
        self.name = name
        self.type = type
        self.arguments = arguments
        self.id = "call_123"

class FakeDelta:
    def __init__(self, type, text):
        self.type = type
        self.text = text

class FakeInteraction:
    def __init__(self):
        self.id = "session_123"

async def fake_stream_1(*args, **kwargs):
    yield FakeEvent("interaction.created", interaction=FakeInteraction())
    yield FakeEvent("step.start", step=FakeStep("sandbox_execute", arguments={"instruction": "Initialize database schema", "substrate": "microVM"}))
    yield FakeEvent("step.stop")
    yield FakeEvent("interaction.completed")

async def fake_stream_2(*args, **kwargs):
    yield FakeEvent("step.delta", delta=FakeDelta("text", "Successfully processed: Initialize database schema"))
    yield FakeEvent("interaction.completed")

@pytest.mark.asyncio
@patch("aura_sdk.agent.genai.Client")
async def test_read_switch_execute_return(mock_client_cls):
    mock_client = MagicMock()
    mock_interactions = AsyncMock()
    mock_interactions.create.side_effect = [fake_stream_1(), fake_stream_2()]
    mock_client.aio.interactions = mock_interactions
    mock_client_cls.return_value = mock_client

    svid = "spiffe://test/agent/worker"
    agent = Agent("WorkerAgent", svid)
    await agent.boot()
    
    # We will simulate the Rust microkernel receiving the dispatch and sending a completion
    def mock_rust_kernel_callback(sample):
        payload = json.loads(sample.payload.decode("utf-8"))
        task_id = payload["task_id"]
        instruction = payload["instruction"]
        
        assert payload["substrate"] == "microVM"
        
        # Publish the completion result back
        completion_payload = {
            "task_id": task_id,
            "result": f"Successfully processed: {instruction}"
        }
        
        # Publish to the completion topic
        agent.transport.publish("tasks/completion", json.dumps(completion_payload))

    # The Rust kernel subscribes to dispatch
    kernel_sub = agent.transport.subscribe("tasks/dispatch", mock_rust_kernel_callback)
    
    # Execute task. The agent should suspend, and the mock kernel will resolve it.
    result = await agent.execute_task("Please use the sandbox_execute tool with substrate='microVM' to run the instruction 'Initialize database schema'")
    
    assert "Successfully processed: Initialize database schema" in result
    
    kernel_sub.undeclare()
    agent.shutdown()
