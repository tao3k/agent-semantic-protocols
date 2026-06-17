"""Two-phase semantic recall plus Q-value reranking."""

from __future__ import annotations

from dataclasses import dataclass

from .encoder import IntentEncoder
from .episode import Episode
from .q_table import QTable


@dataclass(frozen=True)
class TwoPhaseConfig:
    k1: int = 20
    k2: int = 5
    lambda_: float = 0.3


def calculate_score(similarity: float, q_value: float, lambda_: float) -> float:
    return (1.0 - lambda_) * similarity + lambda_ * q_value


class TwoPhaseSearch:
    def __init__(
        self,
        q_table: QTable,
        encoder: IntentEncoder,
        config: TwoPhaseConfig | None = None,
    ) -> None:
        self.q_table = q_table
        self.encoder = encoder
        self.config = config or TwoPhaseConfig()

    def search(
        self,
        episodes: list[Episode],
        intent: str,
        *,
        k1: int | None = None,
        k2: int | None = None,
        lambda_: float | None = None,
    ) -> list[tuple[Episode, float]]:
        phase_one = k1 or self.config.k1
        phase_two = k2 or self.config.k2
        weight = self.config.lambda_ if lambda_ is None else float(lambda_)
        embedding = self.encoder.encode(intent)
        candidates = [
            (episode, self.encoder.cosine_similarity(embedding, episode.intent_embedding))
            for episode in episodes
        ]
        candidates.sort(key=lambda item: item[1], reverse=True)
        reranked = [
            (
                episode,
                calculate_score(similarity, self.q_table.get_q(episode.id), weight),
            )
            for episode, similarity in candidates[:phase_one]
        ]
        reranked.sort(key=lambda item: item[1], reverse=True)
        return reranked[:phase_two]

    def quick_search(self, episodes: list[Episode], intent: str) -> list[tuple[Episode, float]]:
        return self.search(episodes, intent)
