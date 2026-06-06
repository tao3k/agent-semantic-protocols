"""Promotion candidates for repeated fuzzy searches in artifact timelines."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from typing import Any

from .artifact_commands import shell_command, target_like


def fzf_promotion_candidates(
    repeat_groups: Sequence[Mapping[str, Any]],
    *,
    limit: int,
) -> dict[str, object]:
    actions = [
        _action_row(group)
        for group in repeat_groups
        if str(group["method"]) == "search/fzf" and int(group["repeatCount"]) > 0
    ]
    actions.sort(
        key=lambda action: (
            -int(action["repeatCount"]),
            -float(action["spanSeconds"]),
            str(action["language"]),
            str(action["query"]),
        )
    )
    return {
        "policy": "repeat-fzf-to-typed-frontier",
        "replacement": "promote-to-owner-item-test-frontier",
        "promotableSearches": sum(int(action["repeatCount"]) for action in actions),
        "candidateGroupCount": len(actions),
        "actionCount": len(actions),
        "actions": actions[:limit],
    }


def _action_row(group: Mapping[str, Any]) -> dict[str, object]:
    language = str(group["language"])
    query = str(group["subject"])
    project_root_arg = str(group.get("projectRootArg") or ".")
    preferred_command = _preferred_command(language, query, project_root_arg)
    return {
        "decision": "promote",
        "policy": "repeat-fzf-to-typed-frontier",
        "replacement": "promote-to-owner-item-test-frontier",
        "reason": "same fuzzy query repeated before converging on a typed frontier",
        "language": language,
        "method": "search/fzf",
        "query": query,
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
            f"asp {language} search fzf <same-query> owner tests --view seeds "
            f"{project_root_arg}"
        ),
        "nextAction": (
            "promote recurring fuzzy hits into owner, item, and test frontier facts"
        ),
    }


def _preferred_command(language: str, query: str, project_root_arg: str) -> str:
    if target_like(query):
        return shell_command(
            (
                "asp",
                language,
                "search",
                "owner",
                query,
                "items",
                "--view",
                "seeds",
                project_root_arg,
            )
        )
    return shell_command(
        (
            "asp",
            language,
            "search",
            "fzf",
            query,
            "owner",
            "tests",
            "--view",
            "seeds",
            project_root_arg,
        )
    )
