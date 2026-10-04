from aura_sdk.transport import ZenohClient


class EphemeralTunnelTool:
    def __init__(self, transport: ZenohClient):
        self.transport = transport

    async def mint_svid(self, agent_name: str) -> str:
        return await self.transport.dispatch_tool("ephemeral-tunnel", {"agent_name": agent_name})
