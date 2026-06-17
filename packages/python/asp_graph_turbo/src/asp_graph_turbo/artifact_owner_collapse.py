"""Collapse candidates for repeated owner searches in artifact timelines."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from typing import Any

from .artifact_commands import shell_command


def owner_collapse_candidates(
    repeat_groups: Sequence[Mapping[str, Any]],
    *,
    limit: int,
) -> dict[str, object]:
    actions = [
        _action_row(group)
        for group in repeat_groups
        if str(group["method"]) == "search/owner" and int(group["repeatCount"]) > 0
    ]
    actions.sort(
        key=lambda action: (
            -int(action["repeatCount"]),
            -float(action["spanSeconds"]),
            str(action["language"]),
            str(action["owner"]),
        )
    )
    return {
        "policy": "repeat-owner-to-item-test-frontier",
        "replacement": "promote-to-owner-query-item-test-frontier",
        "collapsibleSearches": sum(int(action["repeatCount"]) for action in actions),
        "candidateGroupCount": len(actions),
        "actionCount": len(actions),
        "actions": actions[:limit],
    }


def _action_row(group: Mapping[str, Any]) -> dict[str, object]:
    language = str(group["language"])
    owner = str(group["subject"])
    project_root_arg = str(group.get("projectRootArg") or ".")
    preferred_command = shell_command(
        (
            "asp",
            language,
            "search",
            "owner",
            owner,
            "items",
            "--workspace",
            project_root_arg,
            "--view",
            "seeds",
        )
    )
    return {
        "decision": "collapse",
        "policy": "repeat-owner-to-item-test-frontier",
        "replacement": "promote-to-owner-query-item-test-frontier",
        "reason": "same owner searched repeatedly before converging on item/test facts",
        "language": language,
        "method": "search/owner",
        "owner": owner,
        "projectRootArg": project_root_arg,
        "count": int(group["count"]),
        "repeatCount": int(group["repeatCount"]),
        "spanSeconds": float(group["spanSeconds"]),
        "first": group["first"],
        "last": group["last"],
        "targetProfile": "owner-query",
        "profile": "owner-query",
        "preferredCommand": preferred_command,
        "avoidCommand": (
            f"asp {language} search owner {owner} <same-scope> --workspace {project_root_arg}"
        ),
        "nextAction": (
            "return owner-local hot items and covering tests with the first owner frontier"
        ),
    }
