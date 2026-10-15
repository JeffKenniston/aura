"""
Unit tests for BashTool verifying TOOL-BASH-001 and TOOL-BASH-002:
Session continuity, environment variable persistence, working directory persistence,
and multi-session isolation across sequential invocations.
"""

from __future__ import annotations

import asyncio
import os

import pytest

from aura_agents.tools.bash import BashTool


@pytest.mark.asyncio
async def test_bash_tool_environment_variable_persistence():
    """Asserts that exported environment variables persist across sequential calls."""
    tool = BashTool()
    sid = tool.get_session_id()

    # Step 1: Export variable
    out1 = await tool.execute("export AURA_TEST_VAR='AuraPersistentState'", session_id=sid)
    assert out1 == ""

    # Step 2: Read variable in subsequent invocation
    out2 = await tool.execute("echo $AURA_TEST_VAR", session_id=sid)
    assert out2 == "AuraPersistentState"


@pytest.mark.asyncio
async def test_bash_tool_working_directory_persistence():
    """Asserts that current working directory changes persist across sequential calls."""
    tool = BashTool()
    sid = tool.get_session_id()

    # Step 1: cd to /tmp
    await tool.execute("cd /tmp", session_id=sid)

    # Step 2: pwd in subsequent invocation
    out2 = await tool.execute("pwd", session_id=sid)
    assert os.path.realpath(out2) == os.path.realpath("/tmp")


@pytest.mark.asyncio
async def test_bash_tool_session_isolation():
    """Asserts that state is strictly isolated between distinct session IDs."""
    tool = BashTool()
    session_a = "session-alpha"
    session_b = "session-beta"

    # Set variable in Session A
    await tool.execute("export SHARED_KEY='AlphaValue'", session_id=session_a)

    # Check variable in Session B (must be empty)
    out_b = await tool.execute("echo $SHARED_KEY", session_id=session_b)
    assert out_b == ""

    # Verify Session A still retains its value
    out_a = await tool.execute("echo $SHARED_KEY", session_id=session_a)
    assert out_a == "AlphaValue"


@pytest.mark.asyncio
async def test_bash_tool_new_session_resets_state():
    """Asserts that generating a new session resets state."""
    tool = BashTool()
    await tool.execute("export EPHEMERAL_KEY='ResetMe'")
    assert await tool.execute("echo $EPHEMERAL_KEY") == "ResetMe"

    # Reset with new session
    tool.new_session()
    out = await tool.execute("echo $EPHEMERAL_KEY")
    assert out == ""


@pytest.mark.asyncio
async def test_bash_tool_reset_method():
    """Asserts that explicit reset clears state for the session."""
    tool = BashTool()
    sid = "reset-target-session"
    await tool.execute("export TARGET_KEY='ValueBeforeReset'", session_id=sid)
    assert await tool.execute("echo $TARGET_KEY", session_id=sid) == "ValueBeforeReset"

    tool.reset(session_id=sid)
    out = await tool.execute("echo $TARGET_KEY", session_id=sid)
    assert out == ""


@pytest.mark.asyncio
async def test_bash_tool_timeout_enforcement():
    """Asserts that commands exceeding timeout trigger TimeoutError."""
    tool = BashTool(timeout=0.2)
    with pytest.raises(asyncio.TimeoutError):
        await tool.execute("sleep 2")


@pytest.mark.asyncio
async def test_bash_tool_complex_command_execution():
    """Asserts loops, pipes, and compound expressions execute correctly."""
    tool = BashTool()
    out = await tool.execute("echo 'apple\nbanana\ncherry' | grep 'banana'")
    assert out == "banana"
