"""Execute public ASP commands with bounded provider-process cleanup."""

from __future__ import annotations

import os
import signal
import subprocess
import time

from .large_library_runtime_types import CommandResult


def run_public_command(
    command: list[str],
    *,
    stdin: str | None = None,
    timeout_seconds: int,
    env: dict[str, str],
) -> CommandResult:
    process = subprocess.Popen(
        command,
        stdin=subprocess.PIPE if stdin is not None else None,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env=env,
        start_new_session=os.name == "posix",
    )
    try:
        stdout, stderr = process.communicate(input=stdin, timeout=timeout_seconds)
    except subprocess.TimeoutExpired:
        stdout, stderr, process_tree_terminated = terminate_process_tree(process)
        return CommandResult(
            returncode=process.returncode if process.returncode is not None else -1,
            stdout=stdout,
            stderr=stderr,
            timed_out=True,
            process_tree_terminated=process_tree_terminated,
        )
    return CommandResult(
        returncode=process.returncode,
        stdout=stdout,
        stderr=stderr,
        timed_out=False,
        process_tree_terminated=False,
    )


def terminate_process_tree(process: subprocess.Popen[str]) -> tuple[str, str, bool]:
    if os.name != "posix":
        process.terminate()
        return _drain_terminated_process(process, set(), True)

    process_groups, tree_enumerated = descendant_process_groups(process.pid)
    process_groups.add(process.pid)
    signal_process_groups(process_groups, signal.SIGTERM)
    return _drain_terminated_process(process, process_groups, tree_enumerated)


def _drain_terminated_process(
    process: subprocess.Popen[str],
    process_groups: set[int],
    tree_enumerated: bool,
) -> tuple[str, str, bool]:
    try:
        stdout, stderr = process.communicate(timeout=2)
    except subprocess.TimeoutExpired:
        if os.name == "posix":
            signal_process_groups(process_groups, signal.SIGKILL)
        else:
            process.kill()
        stdout, stderr = process.communicate()
    if os.name != "posix":
        return stdout, stderr, True
    survivors = wait_for_process_groups(process_groups, timeout_seconds=2)
    if survivors:
        signal_process_groups(survivors, signal.SIGKILL)
        survivors = wait_for_process_groups(survivors, timeout_seconds=1)
    return stdout, stderr, tree_enumerated and not survivors


def facade_environment(provider_timeout_ms: int) -> dict[str, str]:
    return {
        **os.environ,
        "ASP_NO_AGENT_PLATFORM": "1",
        "ASP_PROVIDER_TIMEOUT_MS": str(provider_timeout_ms),
    }


def descendant_process_groups(root_pid: int) -> tuple[set[int], bool]:
    if os.name != "posix":
        return set(), False
    try:
        completed = subprocess.run(
            ["ps", "-axo", "pid=,ppid=,pgid="],
            text=True,
            capture_output=True,
            check=False,
            timeout=2,
        )
    except (OSError, subprocess.SubprocessError):
        return set(), False
    if completed.returncode != 0:
        return set(), False
    children: dict[int, list[tuple[int, int]]] = {}
    for line in completed.stdout.splitlines():
        fields = line.split()
        if len(fields) != 3:
            continue
        try:
            pid, parent_pid, process_group = (int(field) for field in fields)
        except ValueError:
            continue
        children.setdefault(parent_pid, []).append((pid, process_group))
    pending = [root_pid]
    process_groups: set[int] = set()
    while pending:
        parent_pid = pending.pop()
        for child_pid, process_group in children.get(parent_pid, []):
            process_groups.add(process_group)
            pending.append(child_pid)
    return process_groups, True


def signal_process_groups(process_groups: set[int], signal_number: int) -> None:
    for process_group in process_groups:
        try:
            os.killpg(process_group, signal_number)
        except (PermissionError, ProcessLookupError):
            pass


def wait_for_process_groups(
    process_groups: set[int], *, timeout_seconds: int
) -> set[int]:
    deadline = time.monotonic() + timeout_seconds
    survivors = set(process_groups)
    while survivors and time.monotonic() < deadline:
        survivors = {
            process_group
            for process_group in survivors
            if process_group_exists(process_group)
        }
        if survivors:
            time.sleep(0.02)
    return survivors


def process_group_exists(process_group: int) -> bool:
    try:
        os.killpg(process_group, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return True
