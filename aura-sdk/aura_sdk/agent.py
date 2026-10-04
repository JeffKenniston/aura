from google import genai
from google.genai import types

from .transport import ZenohClient


class Agent:
    def __init__(self, name: str, svid: str, file_search_store_name: str | None = None):
        self.name = name
        self.svid = svid
        self.transport = ZenohClient(svid)
        self.client = genai.Client()
        self.file_search_store_name = file_search_store_name
        self.previous_interaction_id = None
        
    async def boot(self):
        await self.transport.connect()
        
    async def execute_task(self, prompt: str):
        """
        Implements multi-tier routing with Gemini Interactions API,
        SSE streaming, and tool execution interception.
        """
        print(f"[{self.name}] READ: Received task -> {prompt}")
        
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
            "model": "gemini-3.8-flash",
            "input": prompt,
            "stream": True,
            "tools": tools,
        }
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
                    
            elif event.event_type == "interaction.completed":
                print(f"\n[{self.name}] Finished processing task.")
                
        return final_output
        
    def shutdown(self):
        self.transport.close()
