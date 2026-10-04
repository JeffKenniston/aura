import asyncio
import json
import uuid

import zenoh


class ZenohClient:
    """
    Zero-FFI Transport Client connecting the Python orchestration substrate
    to the Rust aura-core microkernel.
    """
    def __init__(self, svid: str):
        self.svid = svid
        sanitized_id = svid.replace("://", "/").replace("//", "/")
        self.prefix = f"aura/workspace/{sanitized_id}"
        self.session = None
        self._pending_futures = {}
        self._loop = None
        self._subscribers = []

    async def connect(self):
        """
        Connects to the Zenoh bus using default configurations.
        """
        print(f"[{self.svid}] Connecting to Zenoh bus...")
        self._loop = asyncio.get_running_loop()
        config = zenoh.Config()
        self.session = zenoh.open(config)
        
        # Setup internal subscriber to listen for task completions
        completion_topic = "tasks/completion"
        sub = self.subscribe(completion_topic, self._on_completion)
        self._subscribers.append(sub)
        
        print(f"[{self.svid}] Successfully connected to Zenoh.")

    def _on_completion(self, sample):
        """
        Internal callback triggered by Zenoh background threads when a task completes.
        Safely dispatches the resolution back to the main asyncio event loop.
        """
        try:
            payload_str = sample.payload.decode("utf-8")
            data = json.loads(payload_str)
            task_id = data.get("task_id")
            result = data.get("result")
            
            if task_id and task_id in self._pending_futures:
                future = self._pending_futures[task_id]
                # Zenoh callbacks execute in a background thread. Must use call_soon_threadsafe.
                self._loop.call_soon_threadsafe(future.set_result, result)
        except Exception as e:  # noqa: BLE001
            print(f"Failed to process completion callback: {e}")

    def publish(self, sub_topic: str, payload: str):
        if not self.session:
            raise RuntimeError("Zenoh session is not connected.")
        full_topic = f"{self.prefix}/{sub_topic}"
        self.session.put(full_topic, payload)

    async def dispatch_task(self, task_payload: dict) -> dict:
        """
        Dispatches a task and suspends the current coroutine until completion.
        (read-switch-execute-return)
        """
        task_id = str(uuid.uuid4())
        task_payload["task_id"] = task_id
        
        future = self._loop.create_future()
        self._pending_futures[task_id] = future
        
        # Dispatch to microkernel/microVM
        payload_str = json.dumps(task_payload)
        self.publish("tasks/dispatch", payload_str)
        
        # Suspend agent until completion
        result = await future
        
        # Cleanup
        del self._pending_futures[task_id]
        return result

    def subscribe(self, sub_topic: str, callback):
        if not self.session:
            raise RuntimeError("Zenoh session is not connected.")
        full_topic = f"{self.prefix}/{sub_topic}"
        sub = self.session.declare_subscriber(full_topic, callback)
        self._subscribers.append(sub)
        return sub

    def close(self):
        for sub in self._subscribers:
            sub.undeclare()
        self._subscribers.clear()
        
        if self.session:
            self.session.close()
            self.session = None
