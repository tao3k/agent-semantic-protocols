"""Batch ranking for Rust-owned Org plan candidates."""

from __future__ import annotations

from time import time

from .plan_context import (
    GLOBAL_PROJECT_SCOPE,
    PlanMemoryContext,
    normalize_plan_sharing,
    normalize_plan_token,
)
from .store import EpisodeStore
from .two_phase import calculate_score


def rank_plan_candidates(
    payload: dict[str, object],
    *,
    store: EpisodeStore,
    intent: str,
    project: str,
    session: str | None = None,
    branch: str | None = None,
    top_k: int = 5,
) -> dict[str, object]:
    memory_index = _PlanMemoryRankIndex(store, intent)
    ranked = [
        _rank_candidate(
            plan,
            memory_index=memory_index,
            intent=intent,
            project=project,
            session=session,
            branch=branch,
        )
        for plan in payload.get("plans", [])
        if isinstance(plan, dict)
    ]
    ranked.sort(key=lambda item: (-float(item["score"]), str(item["id"])))
    return {
        "schemaId": "agent.semantic-protocols.memory-plan-rank",
        "schemaVersion": "1",
        "engine": "asp-memory-engine",
        "ranker": "memory-engine",
        "plans": ranked[: max(0, top_k)],
    }


def _rank_candidate(
    plan: dict[str, object],
    *,
    memory_index: "_PlanMemoryRankIndex",
    intent: str,
    project: str,
    session: str | None,
    branch: str | None,
) -> dict[str, object]:
    properties = plan.get("properties", {})
    if not isinstance(properties, dict):
        properties = {}
    plan_id = str(plan.get("id") or properties.get("PLAN_ID") or properties.get("ID") or "")
    context = _candidate_context(
        properties,
        plan_id=plan_id,
        project=project,
        session=session,
        branch=branch,
    )
    memory_score = memory_index.score(context, plan_id)
    intent_score = _token_overlap(intent, _candidate_text(plan, properties))
    recency_score = _recency_score(float(plan.get("mtime") or 0.0))
    text_score = (0.65 * intent_score) + (0.35 * recency_score)
    score = (0.55 * text_score) + (0.35 * memory_score) + (0.10 * recency_score)
    return {
        "id": plan_id,
        "score": score,
        "textScore": text_score,
        "memoryScore": memory_score,
        "recencyScore": recency_score,
        "intentScore": intent_score,
    }


def _candidate_context(
    properties: dict[object, object],
    *,
    plan_id: str,
    project: str,
    session: str | None,
    branch: str | None,
) -> PlanMemoryContext:
    context = PlanMemoryContext.from_org_properties(
        {str(key): str(value) for key, value in properties.items()}
    )
    return PlanMemoryContext(
        project_id=project or context.project_id,
        session_id=session or context.session_id,
        plan_id=plan_id or context.plan_id,
        branch_id=branch or context.branch_id,
    )


class _PlanMemoryRankIndex:
    def __init__(self, store: EpisodeStore, intent: str) -> None:
        self.global_score = 0.0
        self.project_scores: dict[str, float] = {}
        self.branch_scores: dict[tuple[str, str], float] = {}
        self.session_scores: dict[tuple[str, str], float] = {}
        self.plan_scores: dict[tuple[str, str], list[tuple[object, float]]] = {}
        embedding = store.encoder.encode(intent)
        for episode in store.episodes:
            self._index_episode(store, embedding, episode)

    def score(self, context: PlanMemoryContext, plan_id: str) -> float:
        project_id = normalize_plan_token(context.project_id, GLOBAL_PROJECT_SCOPE)
        resolved_plan_id = _optional_token(plan_id or context.plan_id)
        scores = [self.global_score, self.project_scores.get(project_id, 0.0)]
        if context.branch_id:
            scores.append(self.branch_scores.get((project_id, context.branch_id), 0.0))
        if context.session_id:
            scores.append(
                self.session_scores.get((project_id, context.session_id), 0.0)
            )
        if resolved_plan_id:
            scores.append(self._exact_plan_score(context, project_id, resolved_plan_id))
        return _clamp01(max(scores, default=0.0))

    def _index_episode(
        self, store: EpisodeStore, embedding: tuple[float, ...], episode: object
    ) -> None:
        base_score = _episode_rank_score(store, embedding, episode)
        project_id = normalize_plan_token(
            getattr(episode, "project_id", None), GLOBAL_PROJECT_SCOPE
        )
        plan_id = _optional_token(getattr(episode, "plan_id", None))
        if plan_id:
            self.plan_scores.setdefault((project_id, plan_id), []).append(
                (episode, base_score)
            )
        self._index_shared_episode(project_id, base_score, episode)

    def _index_shared_episode(
        self, project_id: str, base_score: float, episode: object
    ) -> None:
        sharing = normalize_plan_sharing(getattr(episode, "plan_sharing", None))
        if sharing == "global":
            self.global_score = max(
                self.global_score, base_score * _plan_memory_weight(episode, "")
            )
        elif sharing == "project":
            self.project_scores[project_id] = max(
                self.project_scores.get(project_id, 0.0),
                base_score * _plan_memory_weight(episode, ""),
            )
        elif sharing == "branch":
            self._record_branch_score(project_id, base_score, episode)
        elif sharing == "session":
            self._record_session_score(project_id, base_score, episode)

    def _record_branch_score(
        self, project_id: str, base_score: float, episode: object
    ) -> None:
        branch_id = _optional_token(getattr(episode, "branch_id", None))
        if not branch_id:
            return
        key = (project_id, branch_id)
        self.branch_scores[key] = max(
            self.branch_scores.get(key, 0.0),
            base_score * _plan_memory_weight(episode, ""),
        )

    def _record_session_score(
        self, project_id: str, base_score: float, episode: object
    ) -> None:
        session_id = _optional_token(getattr(episode, "session_id", None))
        if not session_id:
            return
        key = (project_id, session_id)
        self.session_scores[key] = max(
            self.session_scores.get(key, 0.0),
            base_score * _plan_memory_weight(episode, ""),
        )

    def _exact_plan_score(
        self, context: PlanMemoryContext, project_id: str, plan_id: str
    ) -> float:
        score = 0.0
        for episode, base_score in self.plan_scores.get((project_id, plan_id), []):
            if context.can_see(episode):
                score = max(score, base_score * _plan_memory_weight(episode, plan_id))
        return score


def _episode_rank_score(
    store: EpisodeStore, embedding: tuple[float, ...], episode: object
) -> float:
    similarity = store.encoder.cosine_similarity(
        embedding, getattr(episode, "intent_embedding", ())
    )
    return _clamp01(calculate_score(similarity, store.q_table.get_q(episode.id), 0.3))


def _candidate_text(plan: dict[str, object], properties: dict[object, object]) -> str:
    return " ".join(
        [
            _display_title(str(plan.get("title") or "")),
            str(properties.get("OBJECTIVE") or ""),
            str(properties.get("NEXT_ACTION") or ""),
            str(properties.get("RECOVERY_REF") or ""),
        ]
    )


def _plan_memory_weight(episode: object, plan_id: str) -> float:
    if plan_id and getattr(episode, "plan_id", None) == plan_id:
        return 1.0
    sharing = str(getattr(episode, "plan_sharing", "") or "").lower()
    if sharing == "session":
        return 0.65
    if sharing == "branch":
        return 0.50
    if sharing == "project":
        return 0.25
    if sharing == "global":
        return 0.15
    return 0.0


def _display_title(title: str) -> str:
    return " ".join(token for token in title.split() if not _is_progress_cookie(token))


def _is_progress_cookie(token: str) -> bool:
    if not token.startswith("[") or not token.endswith("]"):
        return False
    inner = token[1:-1]
    if inner.endswith("%"):
        return inner[:-1].isdigit()
    left, separator, right = inner.partition("/")
    return bool(separator) and left.isdigit() and right.isdigit()


def _token_overlap(left: str, right: str) -> float:
    left_tokens = _tokens(left)
    if not left_tokens:
        return 0.0
    return len(left_tokens & _tokens(right)) / len(left_tokens)


def _tokens(value: str) -> set[str]:
    import re

    return {token for token in re.findall(r"[A-Za-z0-9]+", value.lower()) if len(token) > 1}


def _recency_score(mtime: float) -> float:
    age_days = max(0.0, time() - mtime) / 86_400.0
    return 1.0 / (1.0 + age_days)


def _optional_token(value: object) -> str | None:
    normalized = str(value or "").strip()
    return normalized or None


def _clamp01(value: float) -> float:
    return max(0.0, min(1.0, value))
