"""Q-value table for memory utility tracking."""

from __future__ import annotations


class QTable:
    def __init__(self, default_q: float = 0.5, learning_rate: float = 0.1) -> None:
        self.default_q = float(default_q)
        self.learning_rate = float(learning_rate)
        self._values: dict[str, float] = {}

    def get_q(self, episode_id: str) -> float:
        return self._values.get(episode_id, self.default_q)

    def set_q(self, episode_id: str, value: float) -> None:
        self._values[episode_id] = _clamp01(value)

    def update(self, episode_id: str, reward: float) -> float:
        current = self.get_q(episode_id)
        updated = current + self.learning_rate * (float(reward) - current)
        self.set_q(episode_id, updated)
        return self.get_q(episode_id)

    def to_mapping(self) -> dict[str, float]:
        return dict(self._values)

    def load_mapping(self, values: dict[str, object]) -> None:
        self._values = {str(key): _clamp01(float(value)) for key, value in values.items()}


def _clamp01(value: float) -> float:
    return max(0.0, min(1.0, value))
