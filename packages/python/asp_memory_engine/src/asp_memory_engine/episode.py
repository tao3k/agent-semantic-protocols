"""Episode records for deterministic memory recall."""

from __future__ import annotations

from dataclasses import dataclass, field
from time import time

from .plan_context import (
    GLOBAL_PROJECT_SCOPE,
    PlanMemoryContext,
    normalize_optional_plan_token,
    normalize_plan_sharing,
    normalize_plan_token,
)

GLOBAL_EPISODE_SCOPE = "_global"


def now_ms() -> int:
    return int(time() * 1000)


def normalize_scope(scope: str | None) -> str:
    value = (scope or "").strip()
    return value if value else GLOBAL_EPISODE_SCOPE


@dataclass(frozen=True)
class EpisodeDraft:
    id: str
    intent: str
    intent_embedding: tuple[float, ...]
    experience: str
    outcome: str
    scope: str | None = None
    project_id: str | None = None
    session_id: str | None = None
    plan_id: str | None = None
    branch_id: str | None = None
    plan_sharing: str | None = None

    def with_scope(self, scope: str) -> "EpisodeDraft":
        return EpisodeDraft(
            id=self.id,
            intent=self.intent,
            intent_embedding=self.intent_embedding,
            experience=self.experience,
            outcome=self.outcome,
            scope=scope,
            project_id=self.project_id,
            session_id=self.session_id,
            plan_id=self.plan_id,
            branch_id=self.branch_id,
            plan_sharing=self.plan_sharing,
        )

    def with_plan_context(
        self,
        context: PlanMemoryContext,
        *,
        sharing: str | None = None,
    ) -> "EpisodeDraft":
        return EpisodeDraft(
            id=self.id,
            intent=self.intent,
            intent_embedding=self.intent_embedding,
            experience=self.experience,
            outcome=self.outcome,
            scope=self.scope,
            project_id=context.project_id,
            session_id=context.session_id,
            plan_id=context.plan_id,
            branch_id=context.branch_id,
            plan_sharing=sharing or self.plan_sharing,
        )


@dataclass
class Episode:
    id: str
    intent: str
    intent_embedding: tuple[float, ...]
    experience: str
    outcome: str
    q_value: float = 0.5
    retrieval_count: int = 0
    success_count: int = 0
    failure_count: int = 0
    created_at: int = field(default_factory=now_ms)
    updated_at: int = field(default_factory=now_ms)
    scope: str = GLOBAL_EPISODE_SCOPE
    project_id: str = GLOBAL_PROJECT_SCOPE
    session_id: str | None = None
    plan_id: str | None = None
    branch_id: str | None = None
    plan_sharing: str = "session"

    @classmethod
    def new(cls, draft: EpisodeDraft) -> "Episode":
        return cls(
            id=draft.id,
            intent=draft.intent,
            intent_embedding=tuple(float(value) for value in draft.intent_embedding),
            experience=draft.experience,
            outcome=draft.outcome,
            scope=normalize_scope(draft.scope),
            project_id=normalize_plan_token(draft.project_id, GLOBAL_PROJECT_SCOPE),
            session_id=normalize_optional_plan_token(draft.session_id),
            plan_id=normalize_optional_plan_token(draft.plan_id),
            branch_id=normalize_optional_plan_token(draft.branch_id),
            plan_sharing=normalize_plan_sharing(draft.plan_sharing),
        )

    @classmethod
    def from_mapping(cls, value: dict[str, object]) -> "Episode":
        return cls(
            id=str(value["id"]),
            intent=str(value["intent"]),
            intent_embedding=tuple(float(item) for item in value["intent_embedding"]),
            experience=str(value["experience"]),
            outcome=str(value["outcome"]),
            q_value=float(value.get("q_value", 0.5)),
            retrieval_count=int(value.get("retrieval_count", 0)),
            success_count=int(value.get("success_count", 0)),
            failure_count=int(value.get("failure_count", 0)),
            created_at=int(value.get("created_at", now_ms())),
            updated_at=int(value.get("updated_at", value.get("created_at", now_ms()))),
            scope=normalize_scope(str(value.get("scope", GLOBAL_EPISODE_SCOPE))),
            project_id=normalize_plan_token(
                str(value.get("project_id", GLOBAL_PROJECT_SCOPE)),
                GLOBAL_PROJECT_SCOPE,
            ),
            session_id=normalize_optional_plan_token(value.get("session_id")),
            plan_id=normalize_optional_plan_token(value.get("plan_id")),
            branch_id=normalize_optional_plan_token(value.get("branch_id")),
            plan_sharing=normalize_plan_sharing(str(value.get("plan_sharing", "session"))),
        )

    def to_mapping(self) -> dict[str, object]:
        return {
            "id": self.id,
            "intent": self.intent,
            "intent_embedding": list(self.intent_embedding),
            "experience": self.experience,
            "outcome": self.outcome,
            "q_value": self.q_value,
            "retrieval_count": self.retrieval_count,
            "success_count": self.success_count,
            "failure_count": self.failure_count,
            "created_at": self.created_at,
            "updated_at": self.updated_at,
            "scope": self.scope,
            "project_id": self.project_id,
            "session_id": self.session_id,
            "plan_id": self.plan_id,
            "branch_id": self.branch_id,
            "plan_sharing": self.plan_sharing,
        }

    def normalize_tracking_fields(self) -> None:
        if self.updated_at == 0:
            self.updated_at = self.created_at
        if self.retrieval_count < self.feedback_count:
            self.retrieval_count = self.feedback_count
        self.scope = normalize_scope(self.scope)
        self.project_id = normalize_plan_token(self.project_id, GLOBAL_PROJECT_SCOPE)
        self.session_id = normalize_optional_plan_token(self.session_id)
        self.plan_id = normalize_optional_plan_token(self.plan_id)
        self.branch_id = normalize_optional_plan_token(self.branch_id)
        self.plan_sharing = normalize_plan_sharing(self.plan_sharing)

    @property
    def feedback_count(self) -> int:
        return self.success_count + self.failure_count

    @property
    def total_uses(self) -> int:
        return max(self.retrieval_count, self.feedback_count)

    def utility(self) -> float:
        total = self.feedback_count + 1.0
        success_rate = (self.success_count + 1.0) / total
        return success_rate * self.q_value

    def mark_success(self) -> None:
        self.retrieval_count += 1
        self.success_count += 1
        self.updated_at = now_ms()

    def mark_failure(self) -> None:
        self.retrieval_count += 1
        self.failure_count += 1
        self.updated_at = now_ms()
