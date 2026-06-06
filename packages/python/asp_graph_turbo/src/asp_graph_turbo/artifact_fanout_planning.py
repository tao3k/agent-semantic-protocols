"""Planning candidates for mixed fanout bursts in artifact timelines."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from typing import Any

from .artifact_commands import shell_command, target_like


def fanout_planning_candidates(
    fanout_hotspots: Sequence[Mapping[str, Any]],
    *,
    limit: int,
) -> dict[str, object]:
    actions = [_action_row(hotspot) for hotspot in fanout_hotspots]
    actions = [action for action in actions if action is not None]
    actions.sort(
        key=lambda action: (
            -int(action["fanoutWidth"]),
            -int(action["events"]),
            float(action["spanSeconds"]),
            str(action["start"]),
        )
    )
    return {
        "policy": "mixed-fanout-to-single-profile-frontier",
        "replacement": "route-to-typed-frontier-before-parallel-fanout",
        "routableFanoutBursts": len(actions),
        "avoidableFanoutBranches": sum(
            max(0, int(action["fanoutWidth"]) - 1) for action in actions
        ),
        "candidateGroupCount": len(actions),
        "actionCount": len(actions),
        "actions": actions[:limit],
    }


def _action_row(hotspot: Mapping[str, Any]) -> dict[str, object] | None:
    key = _preferred_fanout_key(hotspot.get("fanoutKeys"))
    if key is None:
        return None
    language = str(key.get("language") or "")
    method = str(key.get("method") or "")
    subject = str(key.get("subject") or "")
    project_root_arg = str(key.get("projectRootArg") or ".")
    if not language or not method or not subject:
        return None
    return {
        "decision": "route",
        "policy": "mixed-fanout-to-single-profile-frontier",
        "replacement": "route-to-typed-frontier-before-parallel-fanout",
        "reason": "mixed fanout started before a typed frontier was selected",
        "profile": "owner-query",
        "route": "single-profile-frontier-before-fanout",
        "language": language,
        "method": method,
        "subject": subject,
        "projectRootArg": project_root_arg,
        "start": hotspot["start"],
        "end": hotspot["end"],
        "spanSeconds": float(hotspot["spanSeconds"]),
        "fanoutWidth": int(hotspot["fanoutWidth"]),
        "events": int(hotspot["events"]),
        "methods": tuple(_method_names(hotspot.get("methods"))),
        "preferredCommand": _preferred_command(
            language,
            method,
            subject,
            project_root_arg,
        ),
    }


def _preferred_fanout_key(value: object) -> Mapping[str, Any] | None:
    if not isinstance(value, list):
        return None
    keys = [item for item in value if isinstance(item, Mapping)]
    return (
        _first_key(keys, "search/owner", require_target=True)
        or _first_key(keys, "search/fzf", require_target=True)
        or _first_key(keys, "search/fzf", require_target=False)
        or _first_key(keys, "search/owner", require_target=False)
    )


def _first_key(
    keys: Sequence[Mapping[str, Any]], method: str, *, require_target: bool
) -> Mapping[str, Any] | None:
    return next(
        (
            key
            for key in keys
            if str(key.get("method") or "") == method
            and (
                not require_target
                or target_like(str(key.get("subject") or ""))
            )
        ),
        None,
    )


def _preferred_command(
    language: str, method: str, subject: str, project_root_arg: str
) -> str:
    if method == "search/owner" or target_like(subject):
        return shell_command(
            (
                "asp",
                language,
                "search",
                "owner",
                subject,
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
            subject,
            "owner",
            "tests",
            "--view",
            "seeds",
            project_root_arg,
        )
    )


def _method_names(value: object) -> tuple[str, ...]:
    if not isinstance(value, Mapping):
        return ()
    return tuple(sorted(str(item) for item in value))
