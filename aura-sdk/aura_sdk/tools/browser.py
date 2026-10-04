from aura_sdk.transport import ZenohClient


class BrowserTool:
    def __init__(self, transport: ZenohClient):
        self.transport = transport

    async def navigate(self, url: str) -> str:
        return await self.transport.dispatch_tool("browser", {"url": url})
