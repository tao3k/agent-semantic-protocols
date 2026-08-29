"""Optimization target classification for artifact timeline audits."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from typing import Any

from .artifact_commands import shell_command, target_like


def optimization_targets(
    repeat_groups: Sequence[Mapping[str, Any]],
    fanout_hotspots: Sequence[Mapping[str, Any]],
    *,
    limit: int,
) -> list[dict[str, object]]:
    repeat_targets = _sorted_targets(_repeat_targets(repeat_groups))
    fanout_targets = _sorted_targets(_fanout_targets(fanout_hotspots))
    repeat_budget = max(1, (limit + 1) // 2)
    selected = [*repeat_targets[:repeat_budget]]
    selected.extend(fanout_targets[: max(0, limit - len(selected))])
    if len(selected) < limit:
        selected.extend(
            _remaining_targets(repeat_targets, fanout_targets, selected, limit)
        )
    return selected[:limit]


def _sorted_targets(targets: Sequence[dict[str, object]]) -> list[dict[str, object]]:
    return sorted(
        targets,
        key=lambda target: (
            -int(target["impactScore"]),
            str(target["category"]),
            str(target["evidence"]),
        ),
    )


def _remaining_targets(
    repeat_targets: Sequence[dict[str, object]],
    fanout_targets: Sequence[dict[str, object]],
    selected: Sequence[dict[str, object]],
    limit: int,
) -> list[dict[str, object]]:
    selected_keys = {str(target["evidence"]) for target in selected}
    remaining = [
        target
        for target in _sorted_targets([*repeat_targets, *fanout_targets])
        if str(target["evidence"]) not in selected_keys
    ]
    return remaining[: max(0, limit - len(selected))]


def _repeat_targets(groups: Sequence[Mapping[str, Any]]) -> list[dict[str, object]]:
    return [_repeat_target(group) for group in groups if int(group["repeatCount"]) > 0]


def _repeat_target(group: Mapping[str, Any]) -> dict[str, object]:
    category, layer, action = _repeat_policy(str(group["method"]))
    repeat_count = int(group["repeatCount"])
    count = int(group["count"])
    subject = str(group["subject"])
    project_root_arg = str(group.get("projectRootArg") or "")
    root_evidence = f" root={project_root_arg}" if project_root_arg else ""
    target = {
        "category": category,
        "layer": layer,
        "severity": _severity(repeat_count),
        "impactScore": (repeat_count * 10) + count,
        "evidence": (
            f"{group['language']}:{group['method']} subject={subject} "
            f"repeat={repeat_count} count={count}{root_evidence}"
        ),
        "nextAction": action,
        "language": group["language"],
        "method": group["method"],
        "subject": subject,
        "projectRootArg": project_root_arg,
        "repeatCount": repeat_count,
    }
    target.update(
        _repeat_route(
            str(group["language"]),
            str(group["method"]),
            subject,
            project_root_arg,
        )
    )
    return target


def _repeat_policy(method: str) -> tuple[str, str, str]:
    if method == "search/prime":
        return (
            "repeat-prime",
            "agent-guidance-and-cache",
            "Suppress repeated prime calls after a session already has a fresh prime frontier.",
        )
    if method == "search/typed-frontier":
        return (
            "repeat-search",
            "target-recall-and-profile-selection",
            "Promote recurring fuzzy targets into typed owner/item/test frontier facts.",
        )
    if method == "search/owner":
        return (
            "repeat-owner",
            "owner-frontier-expansion",
            "Return hotter owner-local item/test frontiers so repeated owner searches collapse.",
        )
    return (
        "repeat-search",
        "search-loop-control",
        "Explain and cache repeated search intent so the next round can use a narrower frontier.",
    )


def _fanout_targets(hotspots: Sequence[Mapping[str, Any]]) -> list[dict[str, object]]:
    return [_fanout_target(hotspot) for hotspot in hotspots]


def _fanout_target(hotspot: Mapping[str, Any]) -> dict[str, object]:
    methods = hotspot.get("methods")
    method_names = _method_names(methods)
    category, layer, action = _fanout_policy(method_names)
    fanout_width = int(hotspot["fanoutWidth"])
    events = int(hotspot["events"])
    target = {
        "category": category,
        "layer": layer,
        "severity": _severity(fanout_width),
        "impactScore": (fanout_width * 5) + events,
        "evidence": (
            f"start={hotspot['start']} fanout={fanout_width} "
            f"events={events} methods={','.join(method_names)}"
        ),
        "nextAction": action,
        "start": hotspot["start"],
        "fanoutWidth": fanout_width,
        "events": events,
        "methods": method_names,
    }
    target.update(_fanout_route(hotspot))
    return target


def _method_names(value: object) -> tuple[str, ...]:
    if not isinstance(value, Mapping):
        return ()
    return tuple(sorted(str(item) for item in value))


def _fanout_policy(methods: tuple[str, ...]) -> tuple[str, str, str]:
    if len(methods) > 1:
        return (
            "mixed-fanout",
            "round-planning-and-profile-routing",
            "Choose one typed reasoning profile before launching parallel searches.",
        )
    if methods == ("search/typed-frontier",):
        return (
            "wide-typed-frontier-fanout",
            "query-diversity-and-owner-recall",
            "Use graph turbo rank/diversity to narrow fuzzy fanout before owner expansion.",
        )
    if methods == ("query/owner-items",):
        return (
            "wide-owner-item-query-fanout",
            "batched-owner-item-frontier",
            "Batch owner item expansion or return merged hot item windows from the prior search.",
        )
    return (
        "wide-fanout",
        "fanout-budget-control",
        "Cap same-round fanout and require a ranked frontier explanation before widening.",
    )


def _repeat_route(
    language: str, method: str, subject: str, project_root_arg: str
) -> dict[str, object]:
    root = project_root_arg or "."
    if method == "search/typed-frontier":
        return {
            "profile": "owner-query",
            "route": (
                "path-typed-frontier-to-owner-items"
                if target_like(subject)
                else "typed-frontier-owner-test-seeds"
            ),
            "preferredCommand": _frontier_command(language, method, subject, root),
        }
    if method == "search/owner":
        return {
            "profile": "owner-query",
            "route": "owner-to-item-frontier",
            "preferredCommand": _owner_items_command(language, subject, root),
        }
    if method == "search/prime":
        return {
            "profile": "prime",
            "route": "reuse-prime-frontier",
        }
    return {}


def _fanout_route(hotspot: Mapping[str, Any]) -> dict[str, object]:
    key = _preferred_fanout_key(hotspot.get("fanoutKeys"))
    if key is None:
        return {"profile": "typed-frontier", "route": "rank-before-widening"}
    language = str(key.get("language") or "")
    method = str(key.get("method") or "")
    subject = str(key.get("subject") or "")
    root = str(key.get("projectRootArg") or ".")
    if not language or not method or not subject:
        return {"profile": "typed-frontier", "route": "rank-before-widening"}
    return {
        "profile": "owner-query",
        "route": "single-profile-frontier-before-fanout",
        "preferredCommand": _frontier_command(language, method, subject, root),
    }


def _preferred_fanout_key(value: object) -> Mapping[str, Any] | None:
    if not isinstance(value, list):
        return None
    keys = [item for item in value if isinstance(item, Mapping)]
    return (
        _first_key(keys, "search/owner", require_target=True)
        or _first_key(keys, "search/typed-frontier", require_target=True)
        or _first_key(keys, "search/typed-frontier", require_target=False)
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
            and (not require_target or target_like(str(key.get("subject") or "")))
        ),
        None,
    )


def _frontier_command(
    language: str, method: str, subject: str, project_root_arg: str
) -> str:
    if method == "search/owner" or target_like(subject):
        return _owner_items_command(language, subject, project_root_arg)
    return shell_command(
        (
            "asp",
            language,
            "search",
            "typed-frontier",
            subject,
            "owner",
            "tests",
            "--view",
            "seeds",
            project_root_arg,
        )
    )


def _owner_items_command(language: str, owner: str, project_root_arg: str) -> str:
    return shell_command(
        (
            "asp",
            language,
            "search",
            "owner",
            owner,
            "items",
            "--view",
            "seeds",
            project_root_arg,
        )
    )


def _severity(value: int) -> str:
    if value >= 10:
        return "high"
    if value >= 4:
        return "medium"
    return "low"
