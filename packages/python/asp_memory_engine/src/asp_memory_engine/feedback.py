"""Recall feedback model and plan tuning utilities."""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
from math import isfinite


class RecallFeedbackOutcome(str, Enum):
    SUCCESS = "success"
    FAILURE = "failure"

    @property
    def memory_label(self) -> str:
        return "completed" if self is RecallFeedbackOutcome.SUCCESS else "error"

    @property
    def delta(self) -> float:
        return 1.0 if self is RecallFeedbackOutcome.SUCCESS else -1.0


@dataclass(frozen=True)
class RecallPlanTuning:
    k1: int
    k2: int
    lambda_: float
    min_score: float
    max_context_chars: int


def normalize_feedback_bias(value: float) -> float:
    return max(-1.0, min(1.0, value)) if isfinite(value) else 0.0


def update_feedback_bias(previous: float, outcome: RecallFeedbackOutcome) -> float:
    return normalize_feedback_bias(previous * 0.85 + outcome.delta * 0.15)


def apply_feedback_to_plan_tuning(
    plan: RecallPlanTuning,
    feedback_bias: float,
) -> RecallPlanTuning:
    bias = normalize_feedback_bias(feedback_bias)
    if bias <= -0.25:
        strength = min(-bias, 1.0)
        extra_k2 = 2 if strength >= 0.7 else 1
        return _normalized_plan(
            RecallPlanTuning(
                k1=plan.k1 + extra_k2 * 3,
                k2=plan.k2 + extra_k2,
                lambda_=max(0.0, plan.lambda_ - 0.06 * strength),
                min_score=max(0.01, plan.min_score - 0.05 * strength),
                max_context_chars=min(2400, plan.max_context_chars + round(240 * strength)),
            )
        )
    if bias >= 0.35:
        strength = min(bias, 1.0)
        reduce_k2 = 2 if strength >= 0.7 else 1
        return _normalized_plan(
            RecallPlanTuning(
                k1=max(1, plan.k1 - reduce_k2 * 2),
                k2=max(1, plan.k2 - reduce_k2),
                lambda_=min(1.0, plan.lambda_ + 0.05 * strength),
                min_score=min(0.35, plan.min_score + 0.04 * strength),
                max_context_chars=max(320, plan.max_context_chars - round(160 * strength)),
            )
        )
    return _normalized_plan(plan)


def _normalized_plan(plan: RecallPlanTuning) -> RecallPlanTuning:
    k1 = max(1, plan.k1)
    k2 = max(1, min(plan.k2, k1))
    return RecallPlanTuning(k1, k2, plan.lambda_, plan.min_score, plan.max_context_chars)
