"""Promotion candidates for repeated fuzzy searches in artifact timelines."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from typing import Any

from .artifact_commands import shell_command, target_like


def typed_frontier_promotion_candidates(
    repeat_groups: Sequence[Mapping[str, Any]],
    *,
    limit: int,
) -> dict[str, object]:
    actions = [
        _action_row(group)
        for group in repeat_groups
        if str(group["method"]) == "search/typed-frontier" and int(group["repeatCount"]) > 0
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
        "policy": "repeat-search-to-typed-frontier",
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
        "policy": "repeat-search-to-typed-frontier",
        "replacement": "promote-to-owner-item-test-frontier",
        "reason": "same fuzzy query repeated before converging on a typed frontier",
        "language": language,
        "method": "search/typed-frontier",
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
            f"asp {language} search typed-frontier <same-query> owner tests "
            f"--workspace {project_root_arg} --view seeds"
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
                "--workspace",
                project_root_arg,
                "--view",
                "seeds",
            )
        )
    return shell_command(
        (
            "asp",
            language,
            "search",
            "typed-frontier",
            query,
            "owner",
            "tests",
            "--workspace",
            project_root_arg,
            "--view",
            "seeds",
        )
    )
