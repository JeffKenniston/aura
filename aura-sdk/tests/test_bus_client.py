"""
Tests for BusClient and Zenoh primitives.
Verifies AGT-002, AGT-003, AGT-004, AGT-005, AGT-009, AGT-010, AGT-011,
and BUS-001 - BUS-007.
"""

import asyncio
import json
import time

import pytest

from aura_sdk.bus.client import (
    BusClient,
    BusMessage,
    parse_svid,
    resolve_workspace_path,
    state_checkpoint,
)


def test_svid_parsing_and_workspace_resolution():
    """Verifies AGT-005 and AGT-009: SPIFFE parsing and workspace path resolution."""
    svid = "spiffe://prod.aura/tenant-omega/agent/orchestrator/instance-42"
    parsed = parse_svid(svid)
    assert parsed["trust_domain"] == "prod.aura"
    assert parsed["tenant"] == "tenant-omega"
    assert parsed["kind"] == "agent"
    assert parsed["name"] == "orchestrator"
    assert parsed["instance"] == "instance-42"

    path = resolve_workspace_path(svid)
    assert path == "/workspace/tenant-omega/orchestrator"

    # Client instance helper
    client = BusClient(svid, mock=True)
    assert client.get_workspace_path() == "/workspace/tenant-omega/orchestrator"


def test_bus_message_envelope():
    """Verifies BUS-004: Standard message envelope carries correlation ID, SVID, timestamp, schema version."""
    svid = "spiffe://prod.aura/tenant-1/agent/worker"
    msg = BusMessage(
        correlation_id="cid-1234",
        svid=svid,
        timestamp=time.time(),
        schema_version="1.0",
        payload={"action": "test", "status": "ok"},
    )
    d = msg.to_dict()
    assert d["correlation_id"] == "cid-1234"
    assert d["svid"] == svid
    assert d["schema_version"] == "1.0"
    assert d["payload"]["action"] == "test"

    raw_json = msg.to_json()
    reconstructed = BusMessage.from_raw(raw_json)
    assert reconstructed.correlation_id == "cid-1234"
    assert reconstructed.payload["action"] == "test"


@pytest.mark.asyncio
async def test_bus_primitives_in_memory_mock():
    """Verifies AGT-003, AGT-004, AGT-011: call_tool, dispatch_background, await_event, emit."""
    svid = "spiffe://prod.aura/tenant-alpha/agent/planner"
    client = BusClient(svid, mock=True)
    await client.connect()

    # 1. Test emit and await_event (AGT-003, AGT-004)
    async def publisher_task():
        await asyncio.sleep(0.05)
        client.emit("events/alert", {"level": "info", "msg": "system healthy"}, correlation_id="alert-99")

    asyncio.create_task(publisher_task())
    event_result = await client.await_event("events/alert", correlation_id="alert-99", timeout=2.0)
    assert event_result["level"] == "info"
    assert event_result["msg"] == "system healthy"

    # 2. Test dispatch_background (AGT-003)
    task_id = client.dispatch_background({"cmd": "index_repo", "path": "/workspace/tenant-alpha"})
    assert isinstance(task_id, str) and len(task_id) > 0

    # 3. Test call_tool (read-switch-execute-return, AGT-003, BUS-003)
    async def simulate_host_tool_responder():
        # Listen for tools/execute
        def on_tool_exec(sample):
            data = json.loads(sample.payload.decode("utf-8"))
            cid = data["correlation_id"]
            tool_payload = data["payload"]
            assert tool_payload["tool_name"] == "bash"
            assert tool_payload["arguments"]["command"] == "ls -la"
            # Send completion response back
            response_env = {
                "correlation_id": cid,
                "svid": "spiffe://prod.aura/host/kernel",
                "timestamp": time.time(),
                "schema_version": "1.0",
                "payload": {"result": "total 42\ndrwxr-xr-x 2 root root 4096"},
            }
            client.publish("tools/result", json.dumps(response_env))

        client.subscribe("tools/execute", on_tool_exec)

    await simulate_host_tool_responder()

    tool_result = await client.call_tool("bash", {"command": "ls -la"}, timeout=2.0)
    assert "total 42" in tool_result

    client.close()


@pytest.mark.asyncio
async def test_consumer_idempotency():
    """Verifies BUS-007: Idempotency by correlation ID prevents duplicate side-effects."""
    svid = "spiffe://prod.aura/tenant-beta/agent/worker"
    client = BusClient(svid, mock=True)
    await client.connect()

    assert client._mark_seen("corr-101") is True
    # Second delivery of same correlation ID returns False (duplicate)
    assert client._mark_seen("corr-101") is False

    client.close()


@pytest.mark.asyncio
async def test_state_checkpoint_decorator():
    """Verifies AGT-010: State checkpointing decorator serializes locals to Zenoh bus."""
    svid = "spiffe://prod.aura/tenant-gamma/agent/checkpoint-worker"
    client = BusClient(svid, mock=True)
    await client.connect()

    received_checkpoints = []

    def on_checkpoint(sample):
        data = json.loads(sample.payload.decode("utf-8"))
        received_checkpoints.append(data)

    client.subscribe("control/session/checkpoint", on_checkpoint)

    class WorkerAgent:
        def __init__(self, c: BusClient):
            self.transport = c

        @state_checkpoint(bus_client_attr="transport")
        async def long_running_operation(self, file_path: str, retries: int = 3):
            await asyncio.sleep(0.01)
            return f"Processed {file_path}"

    agent = WorkerAgent(client)
    res = await agent.long_running_operation("/workspace/test.py", retries=5)
    assert res == "Processed /workspace/test.py"

    assert len(received_checkpoints) == 1
    cp = received_checkpoints[0]["payload"]
    assert cp["event"] == "state_checkpoint"
    assert cp["function"] == "long_running_operation"
    assert "'/workspace/test.py'" in cp["args"]["file_path"]

    client.close()
