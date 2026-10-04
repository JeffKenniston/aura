from aura_sdk.transport import ZenohClient


class ASTBlastRadiusTool:
    def __init__(self, transport: ZenohClient):
        self.transport = transport

    async def analyze(self, target_symbol_id: str) -> str:
        result = await self.transport.dispatch_tool("ast-blast-radius", {"target_symbol_id": target_symbol_id})
        
        try:
            count = int(result)
            if count > 10:
                return f"CRITICAL: Blast radius is {count} (exceeds 10). Must escalate to Gemini 3.1 Pro."
            return f"Blast radius is {count}."
        except ValueError:
            return result
