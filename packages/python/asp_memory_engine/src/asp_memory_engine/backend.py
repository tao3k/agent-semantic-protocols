"""Local JSON memory state backend."""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from pathlib import Path

from .episode import Episode


@dataclass(frozen=True)
class MemoryStateSnapshot:
    episodes: tuple[Episode, ...] = ()
    q_values: dict[str, float] = field(default_factory=dict)
    recall_feedback_bias_by_scope: dict[str, float] = field(default_factory=dict)


class LocalMemoryStateStore:
    def __init__(self, path: str | Path) -> None:
        self.path = Path(path)

    @property
    def backend_name(self) -> str:
        return "local"

    def load_snapshot(self) -> MemoryStateSnapshot:
        if not self.path.exists():
            return MemoryStateSnapshot()
        payload = json.loads(self.path.read_text(encoding="utf-8"))
        return MemoryStateSnapshot(
            episodes=tuple(Episode.from_mapping(item) for item in payload.get("episodes", [])),
            q_values={str(k): float(v) for k, v in payload.get("q_values", {}).items()},
            recall_feedback_bias_by_scope={
                str(k): float(v)
                for k, v in payload.get("recall_feedback_bias_by_scope", {}).items()
            },
        )

    def save_snapshot(self, snapshot: MemoryStateSnapshot) -> None:
        self.path.parent.mkdir(parents=True, exist_ok=True)
        payload = {
            "episodes": [episode.to_mapping() for episode in snapshot.episodes],
            "q_values": snapshot.q_values,
            "recall_feedback_bias_by_scope": snapshot.recall_feedback_bias_by_scope,
        }
        self.path.write_text(json.dumps(payload, sort_keys=True, indent=2) + "\n", encoding="utf-8")
