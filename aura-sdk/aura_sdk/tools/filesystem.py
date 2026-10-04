from aura_sdk.transport import ZenohClient


class FileSystemTool:
    def __init__(self, transport: ZenohClient):
        self.transport = transport

    async def read(self, path: str) -> str:
        return await self.transport.dispatch_tool("filesystem", {"action": "read", "path": path})

    async def write(self, path: str, content: str) -> str:
        return await self.transport.dispatch_tool("filesystem", {"action": "write", "path": path, "content": content})
