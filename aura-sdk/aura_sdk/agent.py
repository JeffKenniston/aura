import asyncio
from .transport import ZenohClient

class Agent:
    def __init__(self, name: str, svid: str):
        self.name = name
        self.transport = ZenohClient(svid)
        
    async def boot(self):
        await self.transport.connect()
        
    async def execute_task(self, prompt: str):
        """
        Implements the read-switch-execute-return lifecycle.
        """
        print(f"[{self.name}] READ: Received task -> {prompt}")
        
        task_definition = {
            "agent": self.name,
            "instruction": prompt,
            "substrate": "microVM"
        }
        
        print(f"[{self.name}] SWITCH/EXECUTE: Dispatching to microVM and suspending...")
        
        # Agent suspends here. Control yields to the event loop.
        result = await self.transport.dispatch_task(task_definition)
        
        print(f"[{self.name}] RETURN: Resumed with result -> {result}")
        return result
        
    def shutdown(self):
        self.transport.close()
