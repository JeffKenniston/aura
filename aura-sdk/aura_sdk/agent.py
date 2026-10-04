import asyncio
import json

from google import genai

from .router import route_cognitive_demand
from .transport import ZenohClient


class Agent:
    def __init__(self, name: str, svid: str, file_search_store_name: str | None = None):
        self.name = name
        self.svid = svid
        self.transport = ZenohClient(svid)
        self.client = genai.Client()
        self.file_search_store_name = file_search_store_name
        self.previous_interaction_id = None
        self.compression_required = False
        
    async def boot(self):
        await self.transport.connect()
        
        def on_compress(sample):
            try:
                payload = json.loads(sample.payload.decode("utf-8"))
                if payload.get("action") == "compress":
                    print(f"[{self.name}] CRITICAL: Cognitive Cache Saturation warning received! Flagging for compression.")
                    self.compression_required = True
            except (ValueError, KeyError, TypeError, AttributeError):
                pass
                
        self.transport.subscribe("control/session/compress", on_compress)
        
    async def execute_task(self, prompt: str):
        """
        Implements multi-tier routing with Gemini Interactions API,
        SSE streaming, and tool execution interception.
        """
        
        if self.compression_required and self.previous_interaction_id:
            print(f"[{self.name}] COMPRESSING CONTEXT before executing task...")
            # Summarize existing context
            summary_prompt = "System Context Compression: Summarize the entire conversation history, architectural decisions, and current state into a dense context document."
            summary_kwargs = {
                "input": summary_prompt,
                "model": "gemini-3.5-flash-lite", # use cheap tier for summarization
                "previous_interaction_id": self.previous_interaction_id,
                "stream": True
            }
            summary_text = ""
            summary_stream = await self.client.aio.interactions.create(**summary_kwargs)
            async for ev in summary_stream:
                if ev.event_type == "step.delta" and ev.delta.type == "text":
                    summary_text += ev.delta.text
            
            print(f"[{self.name}] Resetting context with compressed summary.")
            self.previous_interaction_id = None
            self.compression_required = False
            
            # Prepend summary to the prompt
            prompt = f"Previous context summary: {summary_text}\n\nNext instruction: {prompt}"

        print(f"[{self.name}] READ: Received task -> {prompt}")
        
        # Multi-Tier Dynamic Routing
        route_config = route_cognitive_demand(prompt)
        print(f"[{self.name}] ROUTE: Assigned to {route_config}")
        
        tools = []
        if self.file_search_store_name:
            tools.append({"type": "file_search", "file_search_store_names": [self.file_search_store_name]})
            
        sandbox_tool = {
            "type": "function",
            "name": "sandbox_execute",
            "function": {
                "name": "sandbox_execute",
                "description": "Executes a functional or operational task within an isolated sandbox.",
                "parameters": {
                    "type": "OBJECT",
                    "properties": {
                        "instruction": {"type": "STRING"},
                        "substrate": {"type": "STRING"}
                    },
                    "required": ["instruction", "substrate"]
                }
            }
        }
        tools.append(sandbox_tool)
            
        kwargs = {
            "input": prompt,
            "stream": True,
            "tools": tools,
        }
        kwargs.update(route_config)
        if self.previous_interaction_id:
            kwargs["previous_interaction_id"] = self.previous_interaction_id
            
        print(f"[{self.name}] Calling interactions.create...")
        response_stream = await self.client.aio.interactions.create(**kwargs)
        print(f"[{self.name}] interactions.create returned! Iterating stream...")
        
        final_output = ""
        async for event in response_stream:
            print("EVENT:", event.event_type)
            if event.event_type == "step.start":
                print("  STEP TYPE:", event.step.type, "NAME:", getattr(event.step, 'name', None))
            elif event.event_type == "step.delta":
                print("  DELTA TYPE:", event.delta.type)
            
            if event.event_type == "interaction.created":
                self.previous_interaction_id = event.interaction.id
                print(f"[{self.name}] Session ID: {self.previous_interaction_id}")
            
            elif event.event_type == "step.delta":
                if event.delta.type == "text":
                    print(event.delta.text, end="", flush=True)
                    final_output += event.delta.text
                elif event.delta.type == "thought_summary":
                    print(f"\033[90m[Thought]: {event.delta.text}\033[0m\n", end="", flush=True)
                    
            elif event.event_type == "step.start" and event.step.type == "function_call":
                call = event.step
                if call.name == "sandbox_execute":
                    args = call.arguments
                    print(f"\n[{self.name}] SWITCH/EXECUTE: Dispatching '{args['instruction']}' to {args['substrate']}...")
                    task_definition = {
                        "agent": self.name,
                        "instruction": args["instruction"],
                        "substrate": args["substrate"]
                    }
                    result = await self.transport.dispatch_task(task_definition)
                    print(f"[{self.name}] RETURN: Received sandbox_execute result -> {result}")
                    
                    # Now we inject the result back by creating a new interaction with the same id
                    # which is semantically "continuing the stream" from the user perspective
                    new_kwargs = kwargs.copy()
                    new_kwargs["input"] = {
                        "type": "function_result",
                        "call_id": call.id,
                        "name": call.name,
                        "result": result
                    }
                    new_kwargs["previous_interaction_id"] = self.previous_interaction_id
                    
                    # Continue reading from the new stream
                    new_stream = await self.client.aio.interactions.create(**new_kwargs)
                    async for new_event in new_stream:
                        if new_event.event_type == "step.delta":
                            if new_event.delta.type == "text":
                                print(new_event.delta.text, end="", flush=True)
                                final_output += new_event.delta.text
                            elif new_event.delta.type == "thought_summary":
                                print(f"\033[90m[Thought]: {new_event.delta.text}\033[0m\n", end="", flush=True)
                        elif new_event.event_type == "interaction.completed":
                            print(f"\n[{self.name}] Finished processing task.")
                            
                            # Emit Telemetry
                            if hasattr(new_event.interaction, 'usage') and new_event.interaction.usage:
                                total_tokens = getattr(new_event.interaction.usage, 'total_tokens', 0)
                                if total_tokens > 0:
                                    metric = {
                                        "agent": self.name,
                                        "session_id": self.previous_interaction_id or "",
                                        "total_tokens": total_tokens
                                    }
                                    self.transport.publish("tasks/metrics/tokens", json.dumps(metric))
                    
            elif event.event_type == "interaction.completed":
                print(f"\n[{self.name}] Finished processing task.")
                
                # Emit Telemetry
                if hasattr(event.interaction, 'usage') and event.interaction.usage:
                    total_tokens = getattr(event.interaction.usage, 'total_tokens', 0)
                    if total_tokens > 0:
                        metric = {
                            "agent": self.name,
                            "session_id": self.previous_interaction_id or "",
                            "total_tokens": total_tokens
                        }
                        self.transport.publish("tasks/metrics/tokens", json.dumps(metric))
                
    async def execute_background_task(self, prompt: str):
        """
        Executes a long-horizon task autonomously in the cloud via background=True.
        Subscribes to Zenoh webhook bridge for completion notifications to avoid polling.
        """
        print(f"[{self.name}] BACKGROUND READ: Received long-horizon task -> {prompt}")
        
        route_config = route_cognitive_demand(prompt)
        
        kwargs = {
            "input": prompt,
            "background": True,
        }
        kwargs.update(route_config)
        if self.previous_interaction_id:
            kwargs["previous_interaction_id"] = self.previous_interaction_id
            
        print(f"[{self.name}] Calling interactions.create with background=True...")
        interaction = await self.client.aio.interactions.create(**kwargs)
        self.previous_interaction_id = interaction.id
        print(f"[{self.name}] Session ID: {self.previous_interaction_id}. Yielding to OS...")

        # Subscribe to Zenoh for status completion
        future = asyncio.get_running_loop().create_future()
        
        def on_status_update(sample):
            try:
                payload_str = sample.payload.decode("utf-8")
                data = json.loads(payload_str)
                status = data.get("status")
                print(f"[{self.name}] BACKGROUND STATUS UPDATE: {status}")
                if status in ("completed", "failed", "requires_action"):
                    asyncio.get_running_loop().call_soon_threadsafe(future.set_result, status)
            except Exception as e:
                print(f"Failed to process status update: {e}")
                
        # The topic published by rust is `interactions/{interaction_id}/status`
        sub_topic = f"interactions/{self.previous_interaction_id}/status"
        sub = self.transport.subscribe(sub_topic, on_status_update)
        
        # Suspend agent until Zenoh bridge sends terminal state
        final_status = await future
        
        # Unsubscribe
        # In Zenoh 1.0/0.11 python API, undeclare() removes the subscription
        sub.undeclare()
        
        # Fetch the completed interaction
        print(f"[{self.name}] Resuming execution and fetching final interaction state.")
        completed_interaction = await self.client.aio.interactions.get(self.previous_interaction_id)
        
        return f"Interaction finished with status: {final_status}"

    def shutdown(self):
        self.transport.close()
