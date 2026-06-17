"""Validate Claude SDK sandtable permission guards."""

from __future__ import annotations

import asyncio

from tools.semantic_sandtable.claude_sdk_permissions import (
    asp_bash_permission_for_budget,
)


def test_asp_bash_permission_enforces_command_budget() -> None:
    permission = asp_bash_permission_for_budget(1)

    first = asyncio.run(
        permission("Bash", {"command": "asp rust search prime --workspace . --view seeds"}, None)
    )
    second = asyncio.run(
        permission(
            "Bash",
            {"command": "asp rust search pipe 'Vec' --workspace . --view seeds"},
            None,
        )
    )

    assert first.behavior == "allow"
    assert second.behavior == "deny"
    assert second.interrupt is True
    assert permission.budget_exhausted is True
    assert permission.asp_command_count == 1
    assert "budget is exhausted" in second.message
