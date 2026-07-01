"""Project repeated search groups and fanout hotspots from timeline rows."""

from __future__ import annotations

from .artifact_events import ArtifactEvent
from .artifact_timeline_keys import event_example, fanout_key, key_row
from .artifact_timeline_parameters import timestamp


def repeat_groups(
    events: tuple[ArtifactEvent, ...], examples: int | None
) -> list[dict[str, object]]:
    groups: dict[tuple[str, str, str, str], list[ArtifactEvent]] = {}
    for event in events:
        if event.action and event.method.startswith("search/"):
            groups.setdefault(fanout_key(event), []).append(event)
    rows = [
        {
            **key_row(key),
            "count": len(items),
            "repeatCount": len(items) - 1,
            "first": timestamp(items[0].timestamp),
            "last": timestamp(items[-1].timestamp),
            "spanSeconds": round(items[-1].timestamp - items[0].timestamp, 3),
            "examples": [event_example(event) for event in items[:3]],
        }
        for key, items in sorted(
            groups.items(),
            key=lambda item: (
                -(len(item[1]) - 1),
                -(item[1][-1].timestamp - item[1][0].timestamp),
                item[0],
            ),
        )
        if len(items) > 1
    ]
    return rows if examples is None else rows[:examples]


def fanout_hotspots(
    bursts: list[dict[str, object]], examples: int | None
) -> list[dict[str, object]]:
    rows = [
        _hotspot_row(burst)
        for burst in sorted(
            (burst for burst in bursts if int(burst["fanoutWidth"]) >= 2),
            key=lambda burst: (
                -int(burst["fanoutWidth"]),
                -int(burst["events"]),
                float(burst["spanSeconds"]),
            ),
        )
    ]
    return rows if examples is None else rows[:examples]


def _hotspot_row(burst: dict[str, object]) -> dict[str, object]:
    return {
        "start": burst["start"],
        "end": burst["end"],
        "spanSeconds": burst["spanSeconds"],
        "events": burst["events"],
        "fanoutWidth": burst["fanoutWidth"],
        "methods": burst["methods"],
        "fanoutKeys": burst["fanoutKeys"],
    }
