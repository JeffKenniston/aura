"""
Adversarial empirical stress test harness for BashTool (TOOL-BASH-002).

Systematically tests:
1. Multi-call environment variable persistence, updates, unsetting, and special characters.
2. Sequential working directory changes, nested paths, directories with spaces, and invalid paths.
3. Multi-session isolation across concurrent and interleaved session IDs.
4. new_session() and reset() semantics.
5. Error resilience: non-zero exit codes, syntax errors, and state continuity after errors.
6. High-throughput sequential command pipeline execution.
"""

from __future__ import annotations

import os
import shutil
import tempfile

import pytest

from aura_agents.tools.bash import BashTool


@pytest.mark.asyncio
async def test_rapid_sequential_env_mutations():
    """Asserts that 10 sequential environment variable mutations accumulate correctly."""
    tool = BashTool()
    sid = tool.get_session_id()

    # Sequentially export 10 variables
    for i in range(1, 11):
        out = await tool.execute(f"export KEY_{i}='val_{i}'", session_id=sid)
        assert out == ""

    # Verify all 10 variables in an aggregated call
    check_script = "echo " + " ".join([f"$KEY_{i}" for i in range(1, 11)])
    result = await tool.execute(check_script, session_id=sid)
    expected = " ".join([f"val_{i}" for i in range(1, 11)])
    assert result == expected


@pytest.mark.asyncio
async def test_env_var_update_and_concatenation():
    """Asserts that environment variables can be mutated sequentially across calls."""
    tool = BashTool()
    sid = tool.get_session_id()

    await tool.execute("export PIPELINE_STATE='init'", session_id=sid)
    await tool.execute("export PIPELINE_STATE=\"$PIPELINE_STATE:stage1\"", session_id=sid)
    await tool.execute("export PIPELINE_STATE=\"$PIPELINE_STATE:stage2\"", session_id=sid)
    await tool.execute("export PIPELINE_STATE=\"$PIPELINE_STATE:done\"", session_id=sid)

    final_val = await tool.execute("echo $PIPELINE_STATE", session_id=sid)
    assert final_val == "init:stage1:stage2:done"


@pytest.mark.asyncio
async def test_nested_working_directory_traversal():
    """Asserts deep directory traversal and backtracking across calls."""
    tool = BashTool()
    sid = tool.get_session_id()
    temp_root = tempfile.mkdtemp(prefix="aura_test_nested_")

    try:
        sub_a = os.path.join(temp_root, "level_a")
        sub_b = os.path.join(sub_a, "level_b")
        os.makedirs(sub_b, exist_ok=True)

        await tool.execute(f"cd '{temp_root}'", session_id=sid)
        pwd1 = await tool.execute("pwd", session_id=sid)
        assert os.path.realpath(pwd1) == os.path.realpath(temp_root)

        await tool.execute("cd level_a", session_id=sid)
        pwd2 = await tool.execute("pwd", session_id=sid)
        assert os.path.realpath(pwd2) == os.path.realpath(sub_a)

        await tool.execute("cd level_b", session_id=sid)
        pwd3 = await tool.execute("pwd", session_id=sid)
        assert os.path.realpath(pwd3) == os.path.realpath(sub_b)

        await tool.execute("cd ../..", session_id=sid)
        pwd4 = await tool.execute("pwd", session_id=sid)
        assert os.path.realpath(pwd4) == os.path.realpath(temp_root)
    finally:
        shutil.rmtree(temp_root, ignore_errors=True)


@pytest.mark.asyncio
async def test_invalid_cd_does_not_break_working_directory():
    """Asserts that attempting to cd into a non-existent directory preserves current cwd."""
    tool = BashTool()
    sid = tool.get_session_id()
    temp_dir = tempfile.mkdtemp(prefix="aura_test_cd_preserve_")

    try:
        await tool.execute(f"cd '{temp_dir}'", session_id=sid)
        pwd_before = await tool.execute("pwd", session_id=sid)
        assert os.path.realpath(pwd_before) == os.path.realpath(temp_dir)

        # Attempt to cd to non-existent path
        await tool.execute("cd /path/that/definitely/does/not/exist_aura_9999", session_id=sid)

        pwd_after = await tool.execute("pwd", session_id=sid)
        assert os.path.realpath(pwd_after) == os.path.realpath(temp_dir)
    finally:
        shutil.rmtree(temp_dir, ignore_errors=True)


@pytest.mark.asyncio
async def test_multi_session_isolation_and_interleaving():
    """Asserts strict isolation between 5 interleaved sessions."""
    tool = BashTool()
    sessions = [f"session_{i}" for i in range(5)]

    # Initialize distinct state in each session
    for i, s in enumerate(sessions):
        await tool.execute(f"export SID_NAME='{s}'", session_id=s)
        await tool.execute(f"export SID_INDEX='{i}'", session_id=s)

    # Interleaved assertions: verify each session retains its own state without leaking
    for i, s in enumerate(sessions):
        name = await tool.execute("echo $SID_NAME", session_id=s)
        idx = await tool.execute("echo $SID_INDEX", session_id=s)
        assert name == s
        assert idx == str(i)

    # Verify a brand new session sees nothing
    fresh_sid = "completely_new_session"
    assert await tool.execute("echo $SID_NAME", session_id=fresh_sid) == ""


@pytest.mark.asyncio
async def test_state_continuity_after_non_zero_exit_codes():
    """Asserts that non-zero exit codes do not destroy or crash the session state."""
    tool = BashTool()
    sid = tool.get_session_id()

    # Step 1: Establish state
    await tool.execute("export SURVIVOR='indestructible'", session_id=sid)
    temp_dir = tempfile.mkdtemp(prefix="aura_err_resilience_")

    try:
        await tool.execute(f"cd '{temp_dir}'", session_id=sid)

        # Step 2: Trigger multiple non-zero exit codes
        await tool.execute("false", session_id=sid)
        await tool.execute("test 1 -eq 2", session_id=sid)
        await tool.execute("ls /nonexistent_aura_test_dir_error_probe", session_id=sid)

        # Step 3: Assert session state is completely preserved
        var_res = await tool.execute("echo $SURVIVOR", session_id=sid)
        assert var_res == "indestructible"

        pwd_res = await tool.execute("pwd", session_id=sid)
        assert os.path.realpath(pwd_res) == os.path.realpath(temp_dir)
    finally:
        shutil.rmtree(temp_dir, ignore_errors=True)


@pytest.mark.asyncio
async def test_syntax_error_resilience():
    """Asserts that commands with bash syntax errors do not crash session continuity."""
    tool = BashTool()
    sid = tool.get_session_id()

    await tool.execute("export BEFORE_SYNTAX='intact'", session_id=sid)

    # Trigger syntax error
    await tool.execute("if then fi", session_id=sid)

    # Verify session survived
    val = await tool.execute("echo $BEFORE_SYNTAX", session_id=sid)
    assert val == "intact"


@pytest.mark.asyncio
async def test_reset_and_reinitialize_session():
    """Asserts that reset clears session and allows fresh reinitialization."""
    tool = BashTool()
    sid = "reinit_session"

    await tool.execute("export LIFECYCLE='active'", session_id=sid)
    assert await tool.execute("echo $LIFECYCLE", session_id=sid) == "active"

    tool.reset(session_id=sid)
    assert await tool.execute("echo $LIFECYCLE", session_id=sid) == ""

    # Re-initialize
    await tool.execute("export LIFECYCLE='reborn'", session_id=sid)
    assert await tool.execute("echo $LIFECYCLE", session_id=sid) == "reborn"


@pytest.mark.asyncio
async def test_high_throughput_sequential_pipeline():
    """Asserts 30 sequential commands execute reliably with cumulative state."""
    tool = BashTool()
    sid = tool.get_session_id()

    await tool.execute("export ACCUMULATOR=0", session_id=sid)
    for _ in range(30):
        await tool.execute("export ACCUMULATOR=$((ACCUMULATOR + 1))", session_id=sid)

    res = await tool.execute("echo $ACCUMULATOR", session_id=sid)
    assert res == "30"


@pytest.mark.asyncio
async def test_adversarial_space_in_working_directory():
    """
    Adversarial test: changing into a directory path containing spaces.
    Evaluates whether subsequent calls retain the working directory when it contains spaces.
    """
    tool = BashTool()
    sid = tool.get_session_id()
    temp_root = tempfile.mkdtemp(prefix="aura space test ")

    try:
        # Move into directory with space
        await tool.execute(f'cd "{temp_root}"', session_id=sid)
        pwd_out = await tool.execute("pwd", session_id=sid)
        assert os.path.realpath(pwd_out) == os.path.realpath(temp_root), (
            f"Expected cwd to persist as {temp_root}, but got {pwd_out}"
        )
    finally:
        shutil.rmtree(temp_root, ignore_errors=True)


@pytest.mark.asyncio
async def test_adversarial_unset_environment_variable():
    """
    Adversarial test: unsetting an exported environment variable.
    Evaluates whether unsetting a variable removes it from subsequent session calls.
    """
    tool = BashTool()
    sid = tool.get_session_id()

    await tool.execute("export UNSET_ME='present'", session_id=sid)
    assert await tool.execute("echo $UNSET_ME", session_id=sid) == "present"

    await tool.execute("unset UNSET_ME", session_id=sid)
    out_after = await tool.execute("echo \"[${UNSET_ME}]\"", session_id=sid)
    assert out_after == "[]", f"Expected UNSET_ME to be cleared, but got {out_after}"


@pytest.mark.asyncio
async def test_adversarial_stderr_capture_on_command_failure():
    """
    Adversarial test: command failure producing stderr and no stdout.
    Evaluates whether the tool returns the stderr error message or silently swallows it as empty string.
    """
    tool = BashTool()
    sid = tool.get_session_id()

    # Command that fails with exit code 2 and prints error message to stderr
    err_out = await tool.execute("ls /nonexistent_aura_file_should_fail_12345", session_id=sid)
    assert err_out != "", "Expected stderr output on failure, but got empty string"
    assert "No such file or directory" in err_out or "cannot access" in err_out
