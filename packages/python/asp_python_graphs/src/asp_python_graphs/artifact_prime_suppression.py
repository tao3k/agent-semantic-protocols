"""Fresh-prime suppression candidates from artifact timelines."""

from __future__ import annotations

from collections.abc import Sequence

from .artifact_events import ArtifactEvent


def prime_suppression_candidates(
    sessions: Sequence[Sequence[ArtifactEvent]],
    *,
    limit: int,
) -> dict[str, object]:
    groups = [
        group
        for session in sessions
        for group in _session_prime_groups(tuple(session))
        if int(group["suppressibleSearches"]) > 0
    ]
    groups.sort(
        key=lambda group: (
            -int(group["suppressibleSearches"]),
            -float(group["spanSeconds"]),
            str(group["language"]),
            str(group["subject"]),
        )
    )
    actions = [action for group in groups for action in group["suppressionActions"]]
    return {
        "policy": "same-session-fresh-prime",
        "freshnessScope": "session+language+subject",
        "replacement": "reuse-prime-frontier",
        "suppressibleSearches": sum(
            int(group["suppressibleSearches"]) for group in groups
        ),
        "candidateGroupCount": len(groups),
        "candidateGroups": groups[:limit],
        "actionCount": len(actions),
        "actions": actions[:limit],
    }


def _session_prime_groups(
    session: tuple[ArtifactEvent, ...],
) -> list[dict[str, object]]:
    groups: dict[tuple[str, str], list[ArtifactEvent]] = {}
    for event in session:
        if event.kind == "search" and event.method == "search/prime":
            groups.setdefault((event.language, _subject(event)), []).append(event)
    return [_group_row(key, items, session) for key, items in groups.items()]


def _group_row(
    key: tuple[str, str],
    items: list[ArtifactEvent],
    session: tuple[ArtifactEvent, ...],
) -> dict[str, object]:
    language, subject = key
    actions = [
        _action_row(items[0], item, language=language, subject=subject)
        for item in items[1:]
    ]
    return {
        "language": language,
        "method": "search/prime",
        "subject": subject,
        "sessionStart": _timestamp(session[0]),
        "sessionEnd": _timestamp(session[-1]),
        "first": _timestamp(items[0]),
        "last": _timestamp(items[-1]),
        "count": len(items),
        "suppressibleSearches": max(0, len(items) - 1),
        "spanSeconds": round(items[-1].timestamp - items[0].timestamp, 3),
        "keptPath": items[0].path,
        "suppressedExamples": [event.path for event in items[1:4]],
        "suppressionActions": actions,
    }


def _action_row(
    kept: ArtifactEvent,
    suppressed: ArtifactEvent,
    *,
    language: str,
    subject: str,
) -> dict[str, object]:
    return {
        "decision": "suppress",
        "policy": "same-session-fresh-prime",
        "replacement": "reuse-prime-frontier",
        "reason": "same language and subject already have a fresh prime frontier",
        "language": language,
        "method": "search/prime",
        "subject": subject,
        "ageSeconds": round(suppressed.timestamp - kept.timestamp, 3),
        "keptAt": _timestamp(kept),
        "suppressedAt": _timestamp(suppressed),
        "keptPath": kept.path,
        "suppressedPath": suppressed.path,
        "avoidCommand": f"asp {language} search prime --workspace . --view seeds",
        "nextAction": "reuse kept prime frontier before running owner/query/read actions",
    }


def _subject(event: ArtifactEvent) -> str:
    return event.target or event.query or event.path.rsplit("/", 1)[-1]


def _timestamp(event: ArtifactEvent) -> str:
    from datetime import datetime

    return datetime.fromtimestamp(event.timestamp).isoformat(timespec="seconds")
