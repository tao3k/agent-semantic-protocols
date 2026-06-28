"""Batch ranking for Rust-owned Org plan candidates."""

from __future__ import annotations

from .plan_context import (
    GLOBAL_PROJECT_SCOPE,
    PlanMemoryContext,
    normalize_plan_sharing,
    normalize_plan_token,
)
from .plan_rank_text import recency_score
from .store import EpisodeStore
from .two_phase import calculate_score


def rank_plan_candidates(
    payload: dict[str, object],
    *,
    store: EpisodeStore,
    project: str,
    session: str | None = None,
    branch: str | None = None,
    top_k: int = 5,
) -> dict[str, object]:
    memory_index = _PlanMemoryRankIndex(store)
    ranked = [
        _rank_candidate(
            plan,
            memory_index=memory_index,
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
    project: str,
    session: str | None,
    branch: str | None,
) -> dict[str, object]:
    properties = plan.get("properties", {})
    if not isinstance(properties, dict):
        properties = {}
    plan_id = str(
        plan.get("id") or properties.get("PLAN_ID") or properties.get("ID") or ""
    )
    context = _candidate_context(
        properties,
        plan_id=plan_id,
        project=project,
        session=session,
        branch=branch,
    )
    checkpoint_signature = _checkpoint_signature(plan)
    memory_score = memory_index.score(
        context,
        plan_id,
        checkpoint_signature=checkpoint_signature,
    )
    context_score = _context_score(
        context, project=project, session=session, branch=branch
    )
    recent = recency_score(float(plan.get("mtime") or 0.0))
    score = (0.50 * context_score) + (0.35 * memory_score) + (0.15 * recent)
    return {
        "id": plan_id,
        "score": score,
        "contextScore": context_score,
        "memoryScore": memory_score,
        "recencyScore": recent,
    }


def _context_score(
    context: PlanMemoryContext,
    *,
    project: str,
    session: str | None,
    branch: str | None,
) -> float:
    if session:
        return 1.0 if context.session_id == session else 0.0
    if branch and context.branch_id == branch:
        return 0.80
    if project and normalize_plan_token(
        context.project_id, GLOBAL_PROJECT_SCOPE
    ) == normalize_plan_token(project, GLOBAL_PROJECT_SCOPE):
        return 0.35
    return 0.0


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
        session_id=context.session_id,
        plan_id=plan_id or context.plan_id,
        branch_id=context.branch_id,
    )


class _PlanMemoryRankIndex:
    def __init__(self, store: EpisodeStore) -> None:
        self.global_score = 0.0
        self.project_scores: dict[str, float] = {}
        self.branch_scores: dict[tuple[str, str], float] = {}
        self.session_scores: dict[tuple[str, str], float] = {}
        self.plan_scores: dict[tuple[str, str], list[tuple[object, float]]] = {}
        self.checkpoints = list(store.checkpoints)
        for episode in store.episodes:
            self._index_episode(store, episode)

    def score(
        self,
        context: PlanMemoryContext,
        plan_id: str,
        *,
        checkpoint_signature: dict[str, object] | None = None,
    ) -> float:
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
        if checkpoint_signature:
            scores.append(
                self._checkpoint_score(
                    context,
                    project_id,
                    resolved_plan_id,
                    checkpoint_signature,
                )
            )
        return _clamp01(max(scores, default=0.0))

    def _index_episode(self, store: EpisodeStore, episode: object) -> None:
        base_score = _episode_rank_score(store, episode)
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

    def _checkpoint_score(
        self,
        context: PlanMemoryContext,
        project_id: str,
        plan_id: str | None,
        checkpoint_signature: dict[str, object],
    ) -> float:
        score = 0.0
        for checkpoint in getattr(self, "checkpoints", []):
            if not _checkpoint_visible(checkpoint, context, project_id, plan_id):
                continue
            score = max(
                score, _checkpoint_match_score(checkpoint, checkpoint_signature)
            )
        return _clamp01(score)


def _episode_rank_score(store: EpisodeStore, episode: object) -> float:
    return _clamp01(calculate_score(1.0, store.q_table.get_q(episode.id), 0.3))


def _checkpoint_signature(plan: dict[str, object]) -> dict[str, object]:
    task_lines: set[int] = set()
    task_keys: set[tuple[str, str, str]] = set()
    tasks = plan.get("taskCandidates", [])
    if isinstance(tasks, list):
        for task in tasks:
            if not isinstance(task, dict):
                continue
            source_line = _optional_int(task.get("sourceLine"))
            if source_line is not None:
                task_lines.add(source_line)
            task_keys.add(
                (
                    str(task.get("kind") or ""),
                    str(task.get("status") or ""),
                    str(task.get("title") or ""),
                )
            )
    return {
        "path": str(plan.get("path") or ""),
        "taskLines": task_lines,
        "taskKeys": task_keys,
    }


def _checkpoint_visible(
    checkpoint: object,
    context: PlanMemoryContext,
    project_id: str,
    plan_id: str | None,
) -> bool:
    checkpoint_project = normalize_plan_token(
        getattr(checkpoint, "project_id", None), GLOBAL_PROJECT_SCOPE
    )
    if checkpoint_project != project_id:
        return False
    checkpoint_session = _optional_token(getattr(checkpoint, "session_id", None))
    if context.session_id and checkpoint_session != context.session_id:
        return False
    checkpoint_branch = _optional_token(getattr(checkpoint, "branch_id", None))
    if (
        context.branch_id
        and checkpoint_branch
        and checkpoint_branch != context.branch_id
    ):
        return False
    checkpoint_plan = _optional_token(getattr(checkpoint, "plan_id", None))
    return bool(plan_id and checkpoint_plan == plan_id) or bool(context.session_id)


def _checkpoint_match_score(
    checkpoint: object,
    checkpoint_signature: dict[str, object],
) -> float:
    plan_path = str(checkpoint_signature.get("path") or "")
    task_lines = checkpoint_signature.get("taskLines")
    if not isinstance(task_lines, set):
        task_lines = set()
    task_keys = checkpoint_signature.get("taskKeys")
    if not isinstance(task_keys, set):
        task_keys = set()

    metadata = getattr(checkpoint, "metadata", {}) or {}
    checkpoint_path = str(metadata.get("planPath") or "")
    checkpoint_line = _optional_int(metadata.get("taskSourceLine"))
    checkpoint_key = (
        str(getattr(checkpoint, "kind", "") or ""),
        str(metadata.get("taskStatus") or ""),
        str(getattr(checkpoint, "title", "") or ""),
    )
    source_locator = str(getattr(checkpoint, "source_locator", "") or "")

    if plan_path and checkpoint_line in task_lines:
        if checkpoint_path == plan_path:
            return 1.0
        if source_locator.startswith(f"{plan_path}:"):
            return 1.0
    if plan_path and checkpoint_path == plan_path:
        return 0.85
    if plan_path and source_locator == plan_path:
        return 0.80
    if checkpoint_key in task_keys:
        return 0.65
    return 0.0


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


def _optional_token(value: object) -> str | None:
    normalized = str(value or "").strip()
    return normalized or None


def _optional_int(value: object) -> int | None:
    try:
        return int(value) if value is not None else None
    except (TypeError, ValueError):
        return None


def _clamp01(value: float) -> float:
    return max(0.0, min(1.0, value))
