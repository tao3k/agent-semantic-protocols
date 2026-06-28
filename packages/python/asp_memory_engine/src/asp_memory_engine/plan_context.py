"""Plan memory visibility context derived from Org plan properties."""

from __future__ import annotations

from dataclasses import dataclass

from .feedback import RecallPlanTuning, apply_feedback_to_plan_tuning, normalize_feedback_bias

GLOBAL_PROJECT_SCOPE = "_global_project"
DEFAULT_BRANCH_SCOPE = "_main"
PLAN_SHARING_MODES = frozenset(
    {"isolated", "plan", "session", "branch", "project", "global"}
)


def normalize_plan_token(value: object, fallback: str) -> str:
    normalized = str(value or "").strip()
    return normalized if normalized else fallback


def normalize_optional_plan_token(value: object) -> str | None:
    normalized = str(value or "").strip()
    return normalized or None


def normalize_plan_sharing(value: object) -> str:
    normalized = normalize_plan_token(value, "session").lower()
    if normalized == "shared":
        return "project"
    if normalized not in PLAN_SHARING_MODES:
        return "session"
    return normalized


def _int_property(properties: dict[str, str], key: str, default: int) -> int:
    try:
        return int(str(properties.get(key, default)).strip())
    except ValueError:
        return default


def _float_property(properties: dict[str, str], key: str, default: float) -> float:
    try:
        return float(str(properties.get(key, default)).strip())
    except ValueError:
        return default


@dataclass(frozen=True)
class PlanRecallComputation:
    scope_key: str
    tuning: RecallPlanTuning
    feedback_bias: float = 0.0

    @classmethod
    def from_org_properties(cls, properties: dict[str, str]) -> "PlanRecallComputation":
        base = RecallPlanTuning(
            k1=_int_property(properties, "MEMORY_RECALL_K1", 20),
            k2=_int_property(properties, "MEMORY_RECALL_K2", 5),
            lambda_=_float_property(properties, "MEMORY_RECALL_LAMBDA", 0.3),
            min_score=_float_property(properties, "MEMORY_MIN_SCORE", 0.12),
            max_context_chars=_int_property(properties, "MEMORY_MAX_CONTEXT_CHARS", 1200),
        )
        feedback_bias = normalize_feedback_bias(
            _float_property(properties, "MEMORY_FEEDBACK_BIAS", 0.0)
        )
        return cls(
            scope_key=normalize_plan_token(
                properties.get("MEMORY_SCOPE"), "_global_memory"
            ),
            tuning=apply_feedback_to_plan_tuning(base, feedback_bias),
            feedback_bias=feedback_bias,
        )


@dataclass(frozen=True)
class PlanMemoryContext:
    project_id: str = GLOBAL_PROJECT_SCOPE
    session_id: str | None = None
    plan_id: str | None = None
    branch_id: str | None = None

    @classmethod
    def from_org_properties(cls, properties: dict[str, str]) -> "PlanMemoryContext":
        return cls(
            project_id=normalize_plan_token(
                properties.get("PLAN_PROJECT") or properties.get("PROJECT_ID"),
                GLOBAL_PROJECT_SCOPE,
            ),
            session_id=normalize_optional_plan_token(
                properties.get("SESSION_ID")
            ),
            plan_id=normalize_optional_plan_token(
                properties.get("PLAN_ID") or properties.get("ID")
            ),
            branch_id=normalize_optional_plan_token(
                properties.get("PLAN_BRANCH") or properties.get("BRANCH_ID")
            ),
        )

    def can_see(self, episode: object) -> bool:
        sharing = normalize_plan_sharing(getattr(episode, "plan_sharing", None))
        if sharing == "global":
            return True
        if normalize_plan_token(
            getattr(episode, "project_id", None), GLOBAL_PROJECT_SCOPE
        ) != normalize_plan_token(self.project_id, GLOBAL_PROJECT_SCOPE):
            return False
        if sharing == "project":
            return True
        if sharing == "branch":
            return bool(
                self.branch_id
                and getattr(episode, "branch_id", None)
                and self.branch_id == getattr(episode, "branch_id")
            )
        if sharing == "session":
            return bool(
                self.session_id
                and getattr(episode, "session_id", None)
                and self.session_id == getattr(episode, "session_id")
            )
        if sharing == "plan":
            return bool(
                self.plan_id
                and getattr(episode, "plan_id", None)
                and self.plan_id == getattr(episode, "plan_id")
            )
        return bool(
            self.plan_id
            and getattr(episode, "plan_id", None)
            and self.plan_id == getattr(episode, "plan_id")
            and (
                self.session_id is None
                or getattr(episode, "session_id", None) is None
                or self.session_id == getattr(episode, "session_id")
            )
        )
