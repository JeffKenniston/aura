from aura_sdk.transport import ZenohClient


class BashTool:
    def __init__(self, transport: ZenohClient):
        self.transport = transport

    async def execute(self, command: str) -> str:
        return await self.transport.dispatch_tool("bash", {"command": command})
