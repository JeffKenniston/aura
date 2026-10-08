"""
Zenoh Event Bus Client and Primitives for Aura SDK.
Implements AGT-002, AGT-003, AGT-004, AGT-005, AGT-009, AGT-010, AGT-011,
BUS-001 - BUS-007.
"""

from __future__ import annotations

import asyncio
import functools
import inspect
import json
import logging
import os
import re
import time
import uuid
from collections import OrderedDict
from collections.abc import Callable
from dataclasses import dataclass, field
from typing import Any

logger = logging.getLogger("aura_sdk.bus")

# Try to import zenoh; fallback safely if not available or mocked
try:
    import zenoh  # type: ignore
    ZENOH_AVAILABLE = True
except ImportError:
    zenoh = None  # type: ignore
    ZENOH_AVAILABLE = False


def parse_svid(svid: str) -> dict[str, str]:
    """
    Parses a SPIFFE ID into its constituent components.
    Format: spiffe://<trust-domain>/<tenant>/<kind>/<name>/<instance>
    """
    pattern = r"^spiffe://([^/]+)/([^/]+)/([^/]+)/([^/]+)(?:/(.+))?$"
    match = re.match(pattern, svid)
    if match:
        return {
            "trust_domain": match.group(1),
            "tenant": match.group(2),
            "kind": match.group(3),
            "name": match.group(4),
            "instance": match.group(5) or "",
        }
    # Fallback for simpler test URIs, e.g. spiffe://test/agent/worker
    parts = svid.replace("spiffe://", "").strip("/").split("/")
    return {
        "trust_domain": parts[0] if len(parts) > 0 else "unknown",
        "tenant": parts[1] if len(parts) > 1 else "default",
        "kind": parts[2] if len(parts) > 2 else "agent",
        "name": parts[3] if len(parts) > 3 else (parts[-1] if len(parts) > 1 else "default"),
        "instance": parts[4] if len(parts) > 4 else "",
    }


def resolve_workspace_path(svid: str) -> str:
    """
    Dynamically resolves and returns the absolute workspace path explicitly
    authorized by the agent's SVID (AGT-009).
    """
    info = parse_svid(svid)
    tenant = info["tenant"]
    name = info["name"]
    return f"/workspace/{tenant}/{name}"


@dataclass
class BusMessage:
    """Standard message envelope required for all Aura bus messages (BUS-004)."""
    correlation_id: str
    svid: str
    timestamp: float
    schema_version: str = "1.0"
    payload: Any = field(default_factory=dict)

    def to_dict(self) -> dict[str, Any]:
        return {
            "correlation_id": self.correlation_id,
            "svid": self.svid,
            "timestamp": self.timestamp,
            "schema_version": self.schema_version,
            "payload": self.payload,
        }

    def to_json(self) -> str:
        return json.dumps(self.to_dict())

    @classmethod
    def from_raw(cls, raw: str | bytes | dict[str, Any], default_svid: str = "") -> BusMessage:
        if isinstance(raw, (str, bytes)):
            if isinstance(raw, bytes):
                raw = raw.decode("utf-8")
            data = json.loads(raw)
        else:
            data = raw

        # Check if envelope structure exists
        if isinstance(data, dict) and "correlation_id" in data and "payload" in data:
            return cls(
                correlation_id=data.get("correlation_id", str(uuid.uuid4())),
                svid=data.get("svid", default_svid),
                timestamp=data.get("timestamp", time.time()),
                schema_version=data.get("schema_version", "1.0"),
                payload=data.get("payload"),
            )
        # Direct payload without envelope wrapper
        correlation_id = ""
        if isinstance(data, dict):
            correlation_id = data.get("task_id") or data.get("tool_id") or data.get("correlation_id") or ""
        return cls(
            correlation_id=correlation_id or str(uuid.uuid4()),
            svid=default_svid,
            timestamp=time.time(),
            schema_version="1.0",
            payload=data,
        )


class BusClient:
    """
    Abstracted interface to Zenoh pub/sub bus (AGT-002, AGT-003).
    Operates strictly as a leaf node to the host router without direct FFI.
    """

    def __init__(self, svid: str, mock: bool = False):
        self.svid = svid
        self.svid_info = parse_svid(svid)
        self.tenant = self.svid_info["tenant"]
        sanitized_id = svid.replace("://", "/").replace("//", "/")
        self.prefix = f"aura/workspace/{sanitized_id}"
        
        # Check mock setting (AGT-011)
        self.is_mock = mock or (os.getenv("AURA_MOCK_BUS") == "1") or not ZENOH_AVAILABLE
        self.session: Any = None
        self._loop: asyncio.AbstractEventLoop | None = None
        self._pending_futures: dict[str, asyncio.Future] = {}
        self._subscribers: list[Any] = []
        
        # Bounded cache for idempotency tracking (BUS-007)
        self._seen_correlation_ids: OrderedDict[str, float] = OrderedDict()
        self._max_seen_ids = 10000
        
        # Mock in-memory pub/sub registry when running in mock mode
        self._mock_subscribers: list[tuple[str, Callable]] = []

    def get_workspace_path(self) -> str:
        """Returns the authorized absolute workspace path (AGT-009)."""
        return resolve_workspace_path(self.svid)

    async def connect(self) -> None:
        """Connects to the Zenoh bus as a leaf node."""
        self._loop = asyncio.get_running_loop()
        logger.info(f"[{self.svid}] Connecting to Zenoh bus (mock={self.is_mock})...")

        if not self.is_mock and ZENOH_AVAILABLE:
            config = zenoh.Config()
            # Default to client / peer mode for leaf node
            self.session = zenoh.open(config)
        else:
            self.session = "mock_session"

        # Subscribe to task completion topic
        self.subscribe("tasks/completion", self._on_completion)

        # Subscribe to tool result topic
        self.subscribe("tools/result", self._on_tool_result)

        logger.info(f"[{self.svid}] Connected successfully.")

    def _mark_seen(self, correlation_id: str) -> bool:
        """
        Records correlation ID for idempotency (BUS-007).
        Returns True if previously unseen, False if duplicate.
        """
        if not correlation_id:
            return True
        if correlation_id in self._seen_correlation_ids:
            return False
        self._seen_correlation_ids[correlation_id] = time.time()
        if len(self._seen_correlation_ids) > self._max_seen_ids:
            self._seen_correlation_ids.popitem(last=False)
        return True

    def _on_completion(self, sample: Any) -> None:
        """Handles completion event callbacks safely across threads."""
        try:
            payload_str = sample.payload.decode("utf-8") if hasattr(sample.payload, "decode") else str(sample.payload)
            data = json.loads(payload_str)
            msg = BusMessage.from_raw(data, default_svid=self.svid)
            
            task_id = None
            result = None
            if isinstance(msg.payload, dict):
                task_id = msg.payload.get("task_id") or msg.correlation_id
                result = msg.payload.get("result", msg.payload)
            else:
                task_id = msg.correlation_id
                result = msg.payload

            if task_id and task_id in self._pending_futures and self._mark_seen(task_id):
                future = self._pending_futures[task_id]
                if not future.done() and self._loop:
                    self._loop.call_soon_threadsafe(future.set_result, result)
        except Exception as e:
            logger.error(f"Error processing completion callback: {e}")

    def _on_tool_result(self, sample: Any) -> None:
        """Handles tool result event callbacks."""
        try:
            payload_str = sample.payload.decode("utf-8") if hasattr(sample.payload, "decode") else str(sample.payload)
            data = json.loads(payload_str)
            msg = BusMessage.from_raw(data, default_svid=self.svid)

            tool_id = None
            result = None
            if isinstance(msg.payload, dict):
                tool_id = msg.payload.get("tool_id") or msg.correlation_id
                result = msg.payload.get("result", msg.payload)
            else:
                tool_id = msg.correlation_id
                result = msg.payload

            if tool_id and tool_id in self._pending_futures and self._mark_seen(tool_id):
                future = self._pending_futures[tool_id]
                if not future.done() and self._loop:
                    self._loop.call_soon_threadsafe(future.set_result, result)
        except Exception as e:
            logger.error(f"Error processing tool result callback: {e}")

    def publish(self, sub_topic: str, payload: str | dict[str, Any]) -> None:
        """Publishes a raw or formatted message to a sub-topic under workspace prefix."""
        full_topic = f"{self.prefix}/{sub_topic}" if not sub_topic.startswith("aura/") else sub_topic
        payload_str = json.dumps(payload) if isinstance(payload, dict) else str(payload)

        if not self.is_mock and self.session and ZENOH_AVAILABLE:
            self.session.put(full_topic, payload_str)
        else:
            # Deliver to mock in-memory subscribers
            self._deliver_mock(full_topic, payload_str)

    def emit(self, topic: str, payload: Any, correlation_id: str | None = None) -> BusMessage:
        """
        Emits an event on the Zenoh bus wrapped in a standard envelope (BUS-004, AGT-003).
        """
        cid = correlation_id or str(uuid.uuid4())
        msg = BusMessage(
            correlation_id=cid,
            svid=self.svid,
            timestamp=time.time(),
            schema_version="1.0",
            payload=payload,
        )
        self.publish(topic, msg.to_json())
        return msg

    async def call_tool(self, tool_name: str, args: dict[str, Any], timeout: float = 30.0) -> Any:
        """
        Invokes an MCP tool asynchronously over the Zenoh bus (AGT-003).
        Suspends the caller using zero CPU until completion arrives (AGT-004).
        """
        if self._loop is None:
            self._loop = asyncio.get_running_loop()

        correlation_id = str(uuid.uuid4())
        future = self._loop.create_future()
        self._pending_futures[correlation_id] = future

        # Key expression per SRS section 7.2: aura/<tenant>/tasks/<tool>/<task_id>
        topic = f"aura/{self.tenant}/tasks/{tool_name}/{correlation_id}"
        req_payload = {
            "tool_id": correlation_id,
            "tool_name": tool_name,
            "arguments": args,
        }
        envelope = BusMessage(
            correlation_id=correlation_id,
            svid=self.svid,
            timestamp=time.time(),
            schema_version="1.0",
            payload=req_payload,
        )
        
        # Publish request to both specific tool topic and fallback tools/execute
        self.publish(topic, envelope.to_json())
        self.publish("tools/execute", envelope.to_json())

        try:
            result = await asyncio.wait_for(future, timeout=timeout)
            return result
        finally:
            self._pending_futures.pop(correlation_id, None)

    def dispatch_background(self, task_payload: dict[str, Any], topic: str | None = None) -> str:
        """
        Dispatches a task in the background without suspending to wait for full completion (AGT-003).
        Returns the unique task ID / correlation ID immediately.
        """
        task_id = str(uuid.uuid4())
        task_payload["task_id"] = task_id
        target_topic = topic or "tasks/dispatch"
        self.emit(target_topic, task_payload, correlation_id=task_id)
        return task_id

    async def await_event(
        self,
        topic: str,
        correlation_id: str | None = None,
        filter_fn: Callable[[dict[str, Any]], bool] | None = None,
        timeout: float = 30.0,
    ) -> Any:
        """
        Suspends the coroutine until an event matching correlation_id/filter_fn arrives (AGT-003, AGT-004).
        Consumes zero CPU while waiting.
        """
        if self._loop is None:
            self._loop = asyncio.get_running_loop()

        future = self._loop.create_future()

        def _on_event(sample: Any) -> None:
            try:
                payload_str = sample.payload.decode("utf-8") if hasattr(sample.payload, "decode") else str(sample.payload)
                data = json.loads(payload_str)
                msg = BusMessage.from_raw(data, default_svid=self.svid)

                if correlation_id:
                    cid = None
                    if isinstance(msg.payload, dict):
                        cid = msg.payload.get("task_id") or msg.payload.get("correlation_id") or msg.correlation_id
                    else:
                        cid = msg.correlation_id
                    if cid != correlation_id:
                        return

                if filter_fn:
                    payload_dict = msg.payload if isinstance(msg.payload, dict) else {"raw": msg.payload}
                    if not filter_fn(payload_dict):
                        return

                if not future.done() and self._loop:
                    self._loop.call_soon_threadsafe(future.set_result, msg.payload)
            except Exception as e:
                logger.error(f"Error in await_event listener: {e}")

        sub = self.subscribe(topic, _on_event)
        try:
            return await asyncio.wait_for(future, timeout=timeout)
        finally:
            if hasattr(sub, "undeclare"):
                sub.undeclare()
            elif sub in self._subscribers:
                self._subscribers.remove(sub)

    async def dispatch_task(self, task_payload: dict[str, Any]) -> dict[str, Any]:
        """
        Backwards-compatible dispatch_task:
        Dispatches a task and suspends current coroutine until completion (read-switch-execute-return).
        """
        if self._loop is None:
            self._loop = asyncio.get_running_loop()

        task_id = str(uuid.uuid4())
        task_payload["task_id"] = task_id

        future = self._loop.create_future()
        self._pending_futures[task_id] = future

        payload_str = json.dumps(task_payload)
        self.publish("tasks/dispatch", payload_str)

        try:
            result = await future
            return result
        finally:
            self._pending_futures.pop(task_id, None)

    async def dispatch_tool(self, tool_type: str, payload: dict[str, Any]) -> str:
        """Backwards-compatible tool execution request."""
        if self._loop is None:
            self._loop = asyncio.get_running_loop()

        tool_id = str(uuid.uuid4())
        req = {
            "tool_id": tool_id,
            "tool_type": tool_type,
            "payload": payload,
        }

        future = self._loop.create_future()
        self._pending_futures[tool_id] = future

        self.publish("tools/execute", json.dumps(req))

        try:
            result = await future
            return result
        finally:
            self._pending_futures.pop(tool_id, None)

    def subscribe(self, sub_topic: str, callback: Callable) -> Any:
        """Declares a subscriber for a topic under prefix or full path."""
        full_topic = f"{self.prefix}/{sub_topic}" if not sub_topic.startswith("aura/") else sub_topic

        if not self.is_mock and self.session and ZENOH_AVAILABLE:
            sub = self.session.declare_subscriber(full_topic, callback)
            self._subscribers.append(sub)
            return sub
        else:
            entry = (full_topic, callback)
            self._mock_subscribers.append(entry)

            class MockSubscription:
                def __init__(self, owner: BusClient, item: tuple):
                    self.owner = owner
                    self.item = item

                def undeclare(self) -> None:
                    if self.item in self.owner._mock_subscribers:
                        self.owner._mock_subscribers.remove(self.item)

            return MockSubscription(self, entry)

    def _deliver_mock(self, topic: str, payload_str: str) -> None:
        """Delivers a message to mock in-memory subscribers matching topic patterns."""
        class MockSample:
            def __init__(self, p: str):
                self.payload = p.encode("utf-8")

        sample = MockSample(payload_str)
        for registered_topic, cb in list(self._mock_subscribers):
            # Simple wildcard matching
            regex = re.escape(registered_topic).replace(r"\*", ".*").replace(r"\+", "[^/]+")
            if re.fullmatch(regex, topic) or topic == registered_topic:
                try:
                    cb(sample)
                except Exception as e:
                    logger.error(f"Mock subscriber error: {e}")

    def resolve_future(self, key_id: str, result: Any) -> None:
        """Manually resolves a pending future (useful in testing/mocking)."""
        if key_id in self._pending_futures:
            future = self._pending_futures[key_id]
            if not future.done():
                if self._loop:
                    self._loop.call_soon_threadsafe(future.set_result, result)
                else:
                    future.set_result(result)

    def close(self) -> None:
        """Closes subscribers and terminates the Zenoh session cleanly."""
        for sub in list(self._subscribers):
            try:
                if hasattr(sub, "undeclare"):
                    sub.undeclare()
            except Exception:
                pass
        self._subscribers.clear()
        self._mock_subscribers.clear()

        # Cancel any pending futures to avoid hanging
        for fut in self._pending_futures.values():
            if not fut.done():
                fut.cancel()
        self._pending_futures.clear()

        if self.session and not self.is_mock and ZENOH_AVAILABLE:
            try:
                self.session.close()
            except Exception:
                pass
        self.session = None


# Backwards compatibility alias
ZenohClient = BusClient


def state_checkpoint(
    bus_client_attr: str = "transport",
    checkpoint_topic: str | None = None,
) -> Callable:
    """
    State-checkpointing decorator (AGT-010).
    Automatically serializes local variables and agent state to the Zenoh bus
    before suspending for long-running tasks.
    """
    def decorator(func: Callable) -> Callable:
        @functools.wraps(func)
        async def wrapper(*args: Any, **kwargs: Any) -> Any:
            # Attempt to locate bus client from first argument (self)
            client: BusClient | None = None
            if args:
                first_arg = args[0]
                if isinstance(first_arg, BusClient):
                    client = first_arg
                elif hasattr(first_arg, bus_client_attr):
                    client = getattr(first_arg, bus_client_attr)

            if client and hasattr(client, "emit"):
                sig = inspect.signature(func)
                bound_args = sig.bind_partial(*args, **kwargs)
                checkpoint_payload = {
                    "event": "state_checkpoint",
                    "function": func.__name__,
                    "args": {k: repr(v) for k, v in bound_args.arguments.items() if k != "self"},
                    "timestamp": time.time(),
                }
                topic = checkpoint_topic or "control/session/checkpoint"
                try:
                    client.emit(topic, checkpoint_payload)
                except Exception as e:
                    logger.warning(f"Failed to emit state checkpoint: {e}")

            return await func(*args, **kwargs)

        return wrapper
    return decorator
