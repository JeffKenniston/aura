"""
Stateful Sandboxed Bash Tool (TOOL-BASH-001, TOOL-BASH-002).
Preserves environment variables and working directory state across sequential invocations
sharing the same session ID within an active task scope.
"""

from __future__ import annotations

import asyncio
import os
import uuid
from typing import Any

from aura_sdk.transport import ZenohClient


class BashTool:
    """
    Sandboxed Bash Tool preserving session continuity across calls.
    """

    def __init__(
        self,
        transport: ZenohClient | None = None,
        default_session_id: str | None = None,
        timeout: float = 125.0,
    ) -> None:
        self.transport = transport
        self.default_session_id = default_session_id or str(uuid.uuid4())
        self.timeout = timeout
        self._sessions: dict[str, dict[str, Any]] = {}

    def get_session_id(self) -> str:
        """Returns the active default session ID."""
        return self.default_session_id

    def new_session(self) -> str:
        """Generates a new session ID, clears old session state, and returns it."""
        self.default_session_id = str(uuid.uuid4())
        return self.default_session_id

    def reset(self, session_id: str | None = None) -> None:
        """Resets the state for a specific session ID or the default session."""
        sid = session_id or self.default_session_id
        self._sessions.pop(sid, None)

    async def execute(self, command: str, session_id: str | None = None) -> str:
        """
        Executes a shell command within the specified session.
        Persists working directory changes and exported environment variables.
        """
        sid = session_id or self.default_session_id

        # 1. Bus dispatch mode when connected to active microkernel
        if self.transport is not None and not getattr(self.transport, "is_mock", True):
            payload = {"command": command, "session_id": sid}
            if hasattr(self.transport, "call_tool"):
                result = await self.transport.call_tool("bash", payload, timeout=self.timeout)
                if isinstance(result, dict):
                    return str(result.get("stdout") or result.get("result", ""))
                return str(result)
            return str(await self.transport.dispatch_tool("bash", payload))

        # 2. Local authentic stateful session tracking (mock/standalone/unit test mode)
        session = self._sessions.setdefault(sid, {
            "cwd": os.getcwd(),
            "env": os.environ.copy(),
        })

        escaped_cwd = session["cwd"].replace('"', '\\"')
        script = (
            f'cd "{escaped_cwd}" 2>/dev/null || true\n'
            f"{command}\n"
            f"__AURA_STATUS__=$?\n"
            f"echo -n '__AURA_EXIT_MARKER__'\n"
            f'echo "$__AURA_STATUS__"\n'
            f"echo -n '__AURA_CWD_MARKER__'\n"
            f"pwd\n"
            f"echo -n '__AURA_ENV_MARKER__'\n"
            f"env -0\n"
        )

        proc = await asyncio.create_subprocess_shell(
            script,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
            env=session["env"],
        )

        stdout_bytes, stderr_bytes = await asyncio.wait_for(
            proc.communicate(),
            timeout=self.timeout,
        )

        if b"__AURA_EXIT_MARKER__" in stdout_bytes:
            parts = stdout_bytes.split(b"__AURA_EXIT_MARKER__", 1)
            user_stdout = parts[0]
            rest = parts[1]

            exit_code = proc.returncode
            if b"__AURA_CWD_MARKER__" in rest:
                exit_part, rest = rest.split(b"__AURA_CWD_MARKER__", 1)
                try:
                    exit_code = int(exit_part.decode("utf-8", errors="replace").strip())
                except ValueError:
                    pass

                if b"__AURA_ENV_MARKER__" in rest:
                    cwd_part, env_part = rest.split(b"__AURA_ENV_MARKER__", 1)
                    new_cwd = cwd_part.decode("utf-8", errors="replace").strip()
                    if os.path.exists(new_cwd):
                        session["cwd"] = new_cwd

                    new_env = {}
                    for item in env_part.split(b"\x00"):
                        if b"=" in item:
                            k, v = item.split(b"=", 1)
                            new_env[k.decode("utf-8", errors="replace")] = v.decode("utf-8", errors="replace")
                    session["env"] = new_env

            output_str = user_stdout.decode("utf-8", errors="replace").rstrip("\n")
            if exit_code != 0 and stderr_bytes and not output_str:
                return stderr_bytes.decode("utf-8", errors="replace").rstrip()
            return output_str

        output_str = stdout_bytes.decode("utf-8", errors="replace").rstrip("\n")
        if proc.returncode != 0 and stderr_bytes and not output_str:
            return stderr_bytes.decode("utf-8", errors="replace").rstrip()
        return output_str
