"""Validate install hook writes the single ASP custom subagent profile."""

from __future__ import annotations

import os
import pathlib
import stat
import subprocess
import sys
import tempfile
import tomllib


STALE_CODEX_PROFILES = [
    "asp-explorer-owner.toml",
    "asp-explorer-rg.toml",
    "asp-explorer-selector.toml",
]

STALE_CLAUDE_PROFILES = [
    "asp-explorer-owner.md",
    "asp-explorer-rg.md",
    "asp-explorer-selector.md",
]


def assert_asp_instructions(instructions: str) -> None:
    assert "Use ASP provider commands before source reads" in instructions
    assert "Parallel fan-out and iterative control are owned by the parent" in instructions
    assert "The shared state is the parent agent's reasoning tree" in instructions
    assert "one instance per reasoning-tree branch" in instructions
    assert "fan in receipts, update the reasoning tree" in instructions
    assert "Instances do not share context windows" in instructions
    assert "action=<action id" in instructions


def assert_codex_profile(root: pathlib.Path, model: str) -> None:
    path = root / ".codex" / "agents" / "asp-explorer.toml"
    agent = tomllib.loads(path.read_text(encoding="utf-8"))
    assert agent["name"] == "asp_explorer"
    assert agent["model"] == model
    assert agent["sandbox_mode"] == "read-only"
    assert_asp_instructions(agent["developer_instructions"])
    for file_name in STALE_CODEX_PROFILES:
        assert not (root / ".codex" / "agents" / file_name).exists()


def assert_claude_profile(root: pathlib.Path, model: str) -> None:
    path = root / ".claude" / "agents" / "asp-explorer.md"
    agent = path.read_text(encoding="utf-8")
    assert "name: asp-explorer" in agent
    assert f"model: '{model}'" in agent
    assert "permissionMode: plan" in agent
    assert_asp_instructions(agent)
    for file_name in STALE_CLAUDE_PROFILES:
        assert not (root / ".claude" / "agents" / file_name).exists()


def emit(line: str) -> None:
    sys.stdout.write(line + "\n")


def write_fake_provider(provider_bin: pathlib.Path) -> None:
    provider_bin.mkdir()
    provider = provider_bin / "rs-harness"
    provider.write_text(
        "#!/bin/sh\n"
        'if [ "$1" = "guide" ]; then\n'
        "  printf '%s\\n' '[agent-guide] runtime=sandtable language=rust provider=rs-harness'\n"
        "fi\n"
        "exit 0\n",
        encoding="utf-8",
    )
    provider.chmod(provider.stat().st_mode | stat.S_IXUSR)


def sandtable_env(root: pathlib.Path) -> dict[str, str]:
    provider_bin = root / ".provider-bin"
    write_fake_provider(provider_bin)
    asp_bin_dir = root / ".agent-bin"
    codex_home = root / ".codex-home"
    env = os.environ.copy()
    env["PATH"] = str(asp_bin_dir) + ":" + str(provider_bin) + ":" + env.get("PATH", "")
    env["SEMANTIC_AGENT_BIN_DIR"] = str(asp_bin_dir)
    env["CODEX_HOME"] = str(codex_home)
    return env


def run_install(asp: pathlib.Path, root: pathlib.Path, env: dict[str, str], *args: str) -> str:
    if args[:2] == ("--client", "codex"):
        command = [str(asp), "install", "plugin", "--codex", *args[2:], str(root)]
    elif args[:2] == ("--client", "claude"):
        command = [str(asp), "install", "hook", "--client", "claude", *args[2:], str(root)]
    else:
        raise ValueError(f"unsupported install args: {args!r}")
    result = subprocess.run(
        command,
        check=True,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    return result.stdout


def write_stale_profiles(root: pathlib.Path, client: str) -> None:
    if client == "codex":
        agent_dir = root / ".codex" / "agents"
        stale_profiles = STALE_CODEX_PROFILES
        contents = 'name = "stale"\n'
    else:
        agent_dir = root / ".claude" / "agents"
        stale_profiles = STALE_CLAUDE_PROFILES
        contents = "---\nname: stale\n---\n"
    agent_dir.mkdir(parents=True, exist_ok=True)
    for file_name in stale_profiles:
        (agent_dir / file_name).write_text(contents, encoding="utf-8")


def validate_codex_default(asp: pathlib.Path, root: pathlib.Path, env: dict[str, str]) -> None:
    write_stale_profiles(root, "codex")
    stdout = run_install(asp, root, env, "--client", "codex")
    assert_codex_profile(root, "gpt-5.3-codex-spark")
    assert "subagent=.codex/agents/asp-explorer.toml" in stdout
    assert "subagents=" not in stdout
    emit("codex-subagent=.codex/agents/asp-explorer.toml")
    emit("codex-model=gpt-5.3-codex-spark")


def validate_codex_override(asp: pathlib.Path, root: pathlib.Path, env: dict[str, str]) -> None:
    stdout = run_install(
        asp,
        root,
        env,
        "--client",
        "codex",
        "--subagent-model",
        "gpt-5.4-mini",
    )
    assert_codex_profile(root, "gpt-5.4-mini")
    assert "subagents=" not in stdout
    emit("codex-override=gpt-5.4-mini")


def validate_claude_default(asp: pathlib.Path, root: pathlib.Path, env: dict[str, str]) -> None:
    write_stale_profiles(root, "claude")
    stdout = run_install(asp, root, env, "--client", "claude")
    assert_claude_profile(root, "haiku")
    assert "subagent=.claude/agents/asp-explorer.md" in stdout
    assert "subagents=" not in stdout
    emit("claude-subagent=.claude/agents/asp-explorer.md")
    emit("claude-model=haiku")


def main() -> None:
    repo = pathlib.Path.cwd()
    asp = repo / "target" / "debug" / "asp"
    if not asp.is_file():
        raise SystemExit("target/debug/asp missing; build asp before running this sandtable")

    with tempfile.TemporaryDirectory(prefix="asp-subagent-sandtable-") as tmp:
        root = pathlib.Path(tmp) / "project"
        root.mkdir()
        (root / ".git").mkdir()
        env = sandtable_env(root)
        validate_codex_default(asp, root, env)
        validate_codex_override(asp, root, env)
        validate_claude_default(asp, root, env)


if __name__ == "__main__":
    main()
