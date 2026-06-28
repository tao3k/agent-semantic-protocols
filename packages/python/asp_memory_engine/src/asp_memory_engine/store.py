"""Episode store with local persistence and scoped recall."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from .backend import LocalMemoryStateStore, MemoryStateSnapshot
from .checkpoint import Checkpoint
from .encoder import IntentEncoder
from .episode import Episode, normalize_scope
from .plan_context import PlanMemoryContext, PlanRecallComputation
from .q_table import QTable
from .two_phase import TwoPhaseSearch


@dataclass(frozen=True)
class StoreConfig:
    path: str = ".data/omni-memory/state.json"
    embedding_dim: int = 384
    table_name: str = "episodes"


class EpisodeStore:
    def __init__(self, config: StoreConfig | None = None) -> None:
        self.config = config or StoreConfig()
        self.encoder = IntentEncoder(self.config.embedding_dim)
        self.q_table = QTable()
        self.episodes: list[Episode] = []
        self.checkpoints: list[Checkpoint] = []
        self.recall_feedback_bias_by_scope: dict[str, float] = {}

    def store(self, episode: Episode) -> str:
        if not episode.id.strip():
            raise ValueError("episode id must not be empty")
        episode.normalize_tracking_fields()
        self.episodes = [item for item in self.episodes if item.id != episode.id]
        self.episodes.append(episode)
        self.q_table.set_q(episode.id, episode.q_value)
        return episode.id

    def store_checkpoint(self, checkpoint: Checkpoint) -> str:
        checkpoint.normalize_tracking_fields()
        if not checkpoint.session_id.strip():
            raise ValueError("checkpoint session_id must not be empty")
        if not checkpoint.title.strip():
            raise ValueError("checkpoint title must not be empty")
        self.checkpoints = [item for item in self.checkpoints if item.id != checkpoint.id]
        self.checkpoints.append(checkpoint)
        return checkpoint.id

    def list_checkpoints(
        self,
        *,
        project_id: str | None = None,
        session_id: str | None = None,
        plan_id: str | None = None,
        branch_id: str | None = None,
        status: str | None = None,
        top_k: int | None = None,
    ) -> list[Checkpoint]:
        checkpoints = [
            checkpoint
            for checkpoint in self.checkpoints
            if checkpoint.matches(
                project_id=project_id,
                session_id=session_id,
                plan_id=plan_id,
                branch_id=branch_id,
                status=status,
            )
        ]
        checkpoints.sort(key=lambda checkpoint: checkpoint.updated_at, reverse=True)
        if top_k is None:
            return checkpoints
        return checkpoints[:top_k]

    def recall(self, intent: str, *, top_k: int = 5, scope: str | None = None) -> list[tuple[Episode, float]]:
        candidates = self._episodes_for_scope(scope)
        return TwoPhaseSearch(self.q_table, self.encoder).search(
            candidates,
            intent,
            k1=max(top_k * 3, top_k),
            k2=top_k,
        )

    def recall_for_plan(
        self,
        intent: str,
        context: PlanMemoryContext,
        *,
        computation: PlanRecallComputation | None = None,
        top_k: int = 5,
    ) -> list[tuple[Episode, float]]:
        candidates = [
            episode for episode in self.episodes if context.can_see(episode)
        ]
        if computation is None:
            return TwoPhaseSearch(self.q_table, self.encoder).search(
                candidates,
                intent,
                k1=max(top_k * 3, top_k),
                k2=top_k,
            )
        results = TwoPhaseSearch(self.q_table, self.encoder).search(
            candidates,
            intent,
            k1=computation.tuning.k1,
            k2=computation.tuning.k2,
            lambda_=computation.tuning.lambda_,
        )
        results = [
            (episode, score)
            for episode, score in results
            if score >= computation.tuning.min_score
        ]
        return _within_context_budget(results, computation.tuning.max_context_chars)

    def recall_with_embedding(
        self,
        embedding: tuple[float, ...],
        *,
        top_k: int = 5,
        scope: str | None = None,
    ) -> list[tuple[Episode, float]]:
        candidates = [
            (episode, self.encoder.cosine_similarity(embedding, episode.intent_embedding))
            for episode in self._episodes_for_scope(scope)
        ]
        candidates.sort(key=lambda item: item[1], reverse=True)
        return candidates[:top_k]

    def snapshot(self) -> MemoryStateSnapshot:
        return MemoryStateSnapshot(
            episodes=tuple(self.episodes),
            checkpoints=tuple(self.checkpoints),
            q_values=self.q_table.to_mapping(),
            recall_feedback_bias_by_scope=dict(self.recall_feedback_bias_by_scope),
        )

    def load_snapshot(self, snapshot: MemoryStateSnapshot) -> None:
        self.episodes = list(snapshot.episodes)
        self.checkpoints = list(snapshot.checkpoints)
        self.q_table.load_mapping(snapshot.q_values)
        self.recall_feedback_bias_by_scope = dict(snapshot.recall_feedback_bias_by_scope)

    def load_state(self, path: str | Path | None = None) -> None:
        self.load_snapshot(LocalMemoryStateStore(path or self.config.path).load_snapshot())

    def save_state(self, path: str | Path | None = None) -> None:
        LocalMemoryStateStore(path or self.config.path).save_snapshot(self.snapshot())

    def _episodes_for_scope(self, scope: str | None) -> list[Episode]:
        if scope is None:
            return list(self.episodes)
        scope_key = normalize_scope(scope)
        return [episode for episode in self.episodes if normalize_scope(episode.scope) == scope_key]


def _within_context_budget(
    results: list[tuple[Episode, float]],
    max_context_chars: int,
) -> list[tuple[Episode, float]]:
    if max_context_chars <= 0:
        return []
    selected: list[tuple[Episode, float]] = []
    used_chars = 0
    for episode, score in results:
        char_cost = len(episode.intent) + len(episode.experience)
        if selected and used_chars + char_cost > max_context_chars:
            continue
        selected.append((episode, score))
        used_chars += char_cost
    return selected
