import pytest
from aura_sdk.agent import Agent

@pytest.mark.anyio
async def test_agent_initialization():
    agent = Agent(name="test_agent", svid="spiffe://aura.local/test_agent")
    assert agent.name == "test_agent"
    assert agent.svid == "spiffe://aura.local/test_agent"

