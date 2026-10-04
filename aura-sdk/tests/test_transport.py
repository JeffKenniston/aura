import asyncio

import pytest

from aura_sdk.transport import ZenohClient


@pytest.mark.asyncio
async def test_zenoh_client_connects():
    svid = "spiffe://test/python"
    client = ZenohClient(svid)
    
    # 1. Connect
    await client.connect()
    assert client.session is not None
    assert client.prefix == "aura/workspace/spiffe/test/python"
    
    # 2. Setup Subscriber
    received_payloads = []
    def on_sample(sample):
        received_payloads.append(sample.payload.decode("utf-8"))

    sub = client.subscribe("events", on_sample)
    
    # 3. Publish
    test_msg = "Hello from Python SDK"
    client.publish("events", test_msg)
    
    # Give Zenoh's background router a tiny moment to deliver
    await asyncio.sleep(0.1)
    
    # 4. Verify
    assert len(received_payloads) == 1
    assert received_payloads[0] == test_msg
    
    # 5. Teardown
    sub.undeclare()
    client.close()
