"""Deterministic memory gate policy."""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum

from .episode import Episode


class MemoryGateDecision(str, Enum):
    RETAIN = "retain"
    OBSOLETE = "obsolete"
    PROMOTE = "promote"


@dataclass(frozen=True)
class MemoryUtilityLedger:
    react_revalidation_score: float
    graph_consistency_score: float
    omega_alignment_score: float
    ttl_score: float
    utility_score: float
    q_value: float
    usage_count: int
    failure_rate: float

    @classmethod
    def from_episode(
        cls,
        episode: Episode,
        react_revalidation_score: float,
        graph_consistency_score: float,
        omega_alignment_score: float,
    ) -> "MemoryUtilityLedger":
        facts = _feedback_facts(episode)
        ttl = _ttl_score(facts)
        react, graph, omega = _evidence_scores(
            react_revalidation_score,
            graph_consistency_score,
            omega_alignment_score,
        )
        q_value = _clamp01(episode.q_value)
        utility = _clamp01(0.32 * react + 0.23 * graph + 0.25 * omega + 0.20 * q_value)
        return cls(
            react,
            graph,
            omega,
            _clamp01(ttl),
            utility,
            q_value,
            facts.usage_count,
            facts.failure_rate,
        )


@dataclass(frozen=True)
class MemoryGatePolicy:
    promote_threshold: float = 0.78
    obsolete_threshold: float = 0.32
    promote_min_usage: int = 3
    obsolete_min_usage: int = 2
    promote_failure_rate_ceiling: float = 0.25
    obsolete_failure_rate_floor: float = 0.70
    promote_min_ttl_score: float = 0.55
    obsolete_max_ttl_score: float = 0.35

    def decide(self, ledger: MemoryUtilityLedger) -> MemoryGateDecision:
        if (
            ledger.utility_score >= self.promote_threshold
            and ledger.usage_count >= self.promote_min_usage
            and ledger.failure_rate <= self.promote_failure_rate_ceiling
            and ledger.ttl_score >= self.promote_min_ttl_score
        ):
            return MemoryGateDecision.PROMOTE
        if (
            ledger.utility_score <= self.obsolete_threshold
            and ledger.usage_count >= self.obsolete_min_usage
            and ledger.failure_rate >= self.obsolete_failure_rate_floor
            and ledger.ttl_score <= self.obsolete_max_ttl_score
        ):
            return MemoryGateDecision.OBSOLETE
        return MemoryGateDecision.RETAIN


def _clamp01(value: float) -> float:
    return max(0.0, min(1.0, float(value)))


@dataclass(frozen=True)
class _FeedbackFacts:
    success: float
    failure: float
    usage_count: int
    failure_rate: float


def _feedback_facts(episode: Episode) -> _FeedbackFacts:
    success = float(episode.success_count)
    failure = float(episode.failure_count)
    feedback = episode.feedback_count
    failure_rate = 1.0 if feedback == 0 and episode.outcome.lower() == "error" else 0.0
    if feedback:
        failure_rate = _clamp01(failure / feedback)
    return _FeedbackFacts(success, failure, episode.total_uses, failure_rate)


def _ttl_score(facts: _FeedbackFacts) -> float:
    frequency = 0.0 if facts.usage_count == 0 else facts.usage_count / (facts.usage_count + 3.0)
    success_bias = (facts.success + 1.0) / (facts.success + facts.failure + 2.0)
    return 0.45 * frequency + 0.35 * (1.0 - facts.failure_rate) + 0.20 * success_bias


def _evidence_scores(react: float, graph: float, omega: float) -> tuple[float, float, float]:
    return _clamp01(react), _clamp01(graph), _clamp01(omega)
