"""Claude SDK tool permission guards for sandtable live-agent runs."""

from __future__ import annotations

import dataclasses
from typing import Any

try:
    from claude_code_sdk.types import PermissionResultAllow, PermissionResultDeny
except ModuleNotFoundError:  # pragma: no cover - exercised through import-only tests.

    @dataclasses.dataclass(frozen=True)
    class PermissionResultAllow:  # type: ignore[no-redef]
        behavior: str = "allow"

    @dataclasses.dataclass(frozen=True)
    class PermissionResultDeny:  # type: ignore[no-redef]
        message: str
        behavior: str = "deny"
        interrupt: bool = False


async def asp_bash_permission(
    tool_name: str,
    tool_input: dict[str, Any],
    context: Any,
) -> PermissionResultAllow | PermissionResultDeny:
    return await AspBashPermission()(tool_name, tool_input, context)


def asp_bash_permission_for_budget(max_commands: int | None) -> "AspBashPermission":
    return AspBashPermission(max_commands=max_commands)


@dataclasses.dataclass(slots=True)
class AspBashPermission:
    max_commands: int | None = None
    asp_command_count: int = 0
    budget_exhausted: bool = False
    async def __call__(
        self,
        tool_name: str,
        tool_input: dict[str, Any],
        _context: Any,
    ) -> PermissionResultAllow | PermissionResultDeny:
        if tool_name != "Bash":
            return PermissionResultDeny(
                message="Use Bash with asp commands only; non-Bash tools are disabled."
            )
        command = str(tool_input.get("command", "")).strip()
        if not is_asp_command(command):
            return PermissionResultDeny(
                message=(
                    "Use asp <language> guide/search/query commands; raw shell "
                    "reads are disabled."
                )
            )
        if (
            self.max_commands is not None
            and self.asp_command_count >= self.max_commands
        ):
            self.budget_exhausted = True
            return PermissionResultDeny(
                message=(
                    "ASP command budget is exhausted for this prompt. Answer from "
                    "the existing ASP frontier, recommendedNext, nextCommand, "
                    "owner, and locator metadata instead of running more commands."
                ),
                interrupt=True,
            )
        self.asp_command_count += 1
        return PermissionResultAllow()


def is_asp_command(command: str) -> bool:
    stripped = command.strip()
    return (
        stripped.startswith("asp ")
        or "/.bin/asp " in stripped
        or stripped.startswith("./.bin/asp ")
        or " && asp " in stripped
        or (
            " direnv exec " in f" {stripped} "
            and (" asp " in f" {stripped} " or " ./.bin/asp " in f" {stripped} ")
        )
    )
