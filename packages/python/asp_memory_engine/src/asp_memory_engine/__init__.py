"""Python adaptation of the Xiuxian memory engine public surface."""

from __future__ import annotations

from .backend import LocalMemoryStateStore, MemoryStateSnapshot
from .encoder import IntentEncoder
from .episode import Episode, EpisodeDraft, GLOBAL_EPISODE_SCOPE
from .plan_context import (
    DEFAULT_BRANCH_SCOPE,
    GLOBAL_PROJECT_SCOPE,
    PLAN_SHARING_MODES,
    PlanMemoryContext,
    PlanRecallComputation,
)
from .feedback import (
    RecallFeedbackOutcome,
    RecallPlanTuning,
    apply_feedback_to_plan_tuning,
    normalize_feedback_bias,
    update_feedback_bias,
)
from .gate import MemoryGateDecision, MemoryGatePolicy, MemoryUtilityLedger
from .inference import (
    InferredMemoryObject,
    InferredMemoryObjectKind,
    infer_memory_object_from_property,
    infer_memory_object_from_reflection,
    infer_memory_object_kind_from_property_key,
    infer_memory_object_kind_from_question,
    infer_memory_objects_from_properties,
)
from .q_table import QTable
from .store import EpisodeStore, StoreConfig
from .two_phase import TwoPhaseConfig, TwoPhaseSearch, calculate_score

__all__ = [
    "Episode",
    "EpisodeDraft",
    "EpisodeStore",
    "DEFAULT_BRANCH_SCOPE",
    "GLOBAL_EPISODE_SCOPE",
    "GLOBAL_PROJECT_SCOPE",
    "InferredMemoryObject",
    "InferredMemoryObjectKind",
    "IntentEncoder",
    "LocalMemoryStateStore",
    "MemoryGateDecision",
    "MemoryGatePolicy",
    "MemoryStateSnapshot",
    "MemoryUtilityLedger",
    "PLAN_SHARING_MODES",
    "PlanMemoryContext",
    "PlanRecallComputation",
    "QTable",
    "RecallFeedbackOutcome",
    "RecallPlanTuning",
    "StoreConfig",
    "TwoPhaseConfig",
    "TwoPhaseSearch",
    "apply_feedback_to_plan_tuning",
    "calculate_score",
    "infer_memory_object_from_property",
    "infer_memory_object_from_reflection",
    "infer_memory_object_kind_from_property_key",
    "infer_memory_object_kind_from_question",
    "infer_memory_objects_from_properties",
    "normalize_feedback_bias",
    "update_feedback_bias",
]
