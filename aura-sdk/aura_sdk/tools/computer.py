from aura_sdk.transport import ZenohClient


class ComputerTool:
    def __init__(self, transport: ZenohClient):
        self.transport = transport

    async def click(self, x: int, y: int) -> str:
        return await self.transport.dispatch_tool("computer", {"x": x, "y": y})
