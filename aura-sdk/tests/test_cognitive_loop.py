import asyncio
import json
import pytest
from aura_sdk.agent import Agent

@pytest.mark.asyncio
async def test_read_switch_execute_return():
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
    result = await agent.execute_task("Initialize database schema")
    
    assert result == "Successfully processed: Initialize database schema"
    
    kernel_sub.undeclare()
    agent.shutdown()
