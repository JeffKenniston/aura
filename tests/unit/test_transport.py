import pytest
import asyncio
from unittest.mock import MagicMock, patch
from aura_sdk.transport import ZenohClient

@pytest.mark.anyio
async def test_zenoh_client():
    client = ZenohClient("spiffe://aura.local/agent/test")
    assert client.prefix == "aura/workspace/spiffe/aura.local/agent/test"

    with patch('zenoh.open') as mock_open:
        mock_session = MagicMock()
        mock_open.return_value = mock_session
        
        await client.connect()
        assert client.session is not None
        
        # Test publish
        client.publish("test/topic", "payload")
        mock_session.put.assert_called_with(f"{client.prefix}/test/topic", "payload")
        
        client.close()
        assert client.session is None
