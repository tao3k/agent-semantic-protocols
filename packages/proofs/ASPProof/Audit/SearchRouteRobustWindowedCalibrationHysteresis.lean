-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import Lean
import ASPProof.SearchRouteRobustWindowedCalibrationHysteresis

namespace ASPProof.Audit.SearchRouteRobustWindowedCalibrationHysteresis

open Lean

private def stringArray (values : Array String) : Json :=
  .arr (values.map Json.str)

private def declaration
    (name theoremFamily description : String)
    (clauseIds axioms : Array String) : Json :=
  .mkObj [
    ("axioms", stringArray axioms),
    ("hasSorryAx", .bool false),
    ("kind", .str "theorem"),
    ("name", .str name),
    ("rfcClauseIds", stringArray clauseIds),
    ("theoremFamily", .str theoremFamily),
    ("type", .str description)
  ]

def declarations : Array Json := #[
  declaration
    "ASPProof.SearchRouteRobustWindowedCalibrationHysteresis.clipped_regret_is_bounded"
    "single-observation-influence-bound"
    "one observation contributes no more than the configured regret cap"
    #["ASP-RFC-10.05-RWCH-CLIP"] #["propext"],
  declaration
    "ASPProof.SearchRouteRobustWindowedCalibrationHysteresis.window_score_is_bounded_by_entry_count"
    "window-score-entry-bound"
    "window score is bounded by entry count multiplied by the regret cap"
    #["ASP-RFC-10.05-RWCH-WINDOW", "ASP-RFC-10.05-RWCH-CLIP"]
    #["Quot.sound", "propext"],
  declaration
    "ASPProof.SearchRouteRobustWindowedCalibrationHysteresis.valid_window_score_is_bounded_by_capacity"
    "window-score-capacity-bound"
    "a valid window score is bounded by capacity multiplied by the regret cap"
    #["ASP-RFC-10.05-RWCH-WINDOW", "ASP-RFC-10.05-RWCH-CLIP"]
    #["Quot.sound", "propext"],
  declaration
    "ASPProof.SearchRouteRobustWindowedCalibrationHysteresis.valid_window_has_unique_source_keys"
    "window-source-diversity"
    "a valid window has no duplicate authority-epoch source key"
    #["ASP-RFC-10.05-RWCH-DIVERSITY"] #[],
  declaration
    "ASPProof.SearchRouteRobustWindowedCalibrationHysteresis.valid_window_contains_only_verified_observations"
    "verified-window-membership"
    "every member of a valid robust window is verified"
    #["ASP-RFC-10.05-RWCH-VERIFIED"] #[],
  declaration
    "ASPProof.SearchRouteRobustWindowedCalibrationHysteresis.high_score_forces_exploration"
    "high-threshold-exploration-entry"
    "a score at or above the high threshold forces exploration from every mode"
    #["ASP-RFC-10.05-RWCH-HYSTERESIS", "ASP-RFC-10.05-RWCH-ENTER"]
    #["Quot.sound", "propext"],
  declaration
    "ASPProof.SearchRouteRobustWindowedCalibrationHysteresis.exploration_persists_without_recovery"
    "exploration-persistence"
    "exploration persists when the low-score recovery condition is absent"
    #["ASP-RFC-10.05-RWCH-HYSTERESIS", "ASP-RFC-10.05-RWCH-EXIT"]
    #["propext"],
  declaration
    "ASPProof.SearchRouteRobustWindowedCalibrationHysteresis.exploration_exit_requires_low_score_and_recovery"
    "exploration-exit-necessity"
    "leaving exploration requires both low score and sufficient recovery streak"
    #["ASP-RFC-10.05-RWCH-EXIT"] #["propext"],
  declaration
    "ASPProof.SearchRouteRobustWindowedCalibrationHysteresis.degraded_mode_is_stable_inside_hysteresis_band"
    "degraded-band-stability"
    "degraded mode remains stable strictly between the low and high thresholds"
    #["ASP-RFC-10.05-RWCH-HYSTERESIS"] #["Quot.sound", "propext"],
  declaration
    "ASPProof.SearchRouteRobustWindowedCalibrationHysteresis.calibration_refresh_preserves_search_cache"
    "search-cache-lane-preservation"
    "calibration refresh preserves graph and semantic search cache identity"
    #["ASP-RFC-10.05-RWCH-CACHE"] #[],
  declaration
    "ASPProof.SearchRouteRobustWindowedCalibrationHysteresis.calibration_refresh_preserves_model_prefix_cache"
    "model-prefix-cache-preservation"
    "calibration refresh preserves stable model-prefix cache identity"
    #["ASP-RFC-10.05-RWCH-PREFIX"] #[],
  declaration
    "ASPProof.SearchRouteRobustWindowedCalibrationHysteresis.calibration_refresh_advances_forecast_plan_version"
    "forecast-plan-cache-version-advance"
    "calibration refresh advances the forecast-plan cache version"
    #["ASP-RFC-10.05-RWCH-CACHE"] #[],
  declaration
    "ASPProof.SearchRouteRobustWindowedCalibrationHysteresis.passive_calibration_adds_no_graph_hops"
    "zero-calibration-graph-hops"
    "passive calibration adds no graph hop"
    #["ASP-RFC-10.05-RWCH-OVERHEAD", "ASP-RFC-10.05-RWCH-ROUTER"] #[],
  declaration
    "ASPProof.SearchRouteRobustWindowedCalibrationHysteresis.passive_calibration_adds_no_provider_commands"
    "zero-calibration-provider-commands"
    "passive calibration adds no provider command"
    #["ASP-RFC-10.05-RWCH-OVERHEAD", "ASP-RFC-10.05-RWCH-ROUTER"] #[],
  declaration
    "ASPProof.SearchRouteRobustWindowedCalibrationHysteresis.passive_calibration_adds_no_llm_rounds"
    "zero-calibration-llm-rounds"
    "passive calibration adds no LLM interaction round"
    #["ASP-RFC-10.05-RWCH-OVERHEAD", "ASP-RFC-10.05-RWCH-ROUTER"] #[],
  declaration
    "ASPProof.SearchRouteRobustWindowedCalibrationHysteresis.passive_calibration_adds_no_disclosed_tokens"
    "zero-calibration-disclosed-tokens"
    "passive calibration adds no disclosed token"
    #["ASP-RFC-10.05-RWCH-OVERHEAD"] #[],
  declaration
    "ASPProof.SearchRouteRobustWindowedCalibrationHysteresis.authorized_robust_batch_requires_valid_window"
    "robust-batch-window-authorization"
    "authorized exploitation requires a valid robust window"
    #["ASP-RFC-10.05-RWCH-WINDOW", "ASP-RFC-10.05-RWCH-AUTHORIZATION"] #[],
  declaration
    "ASPProof.SearchRouteRobustWindowedCalibrationHysteresis.authorized_robust_batch_requires_valid_facts"
    "robust-batch-evidence-authorization"
    "authorized exploitation requires verified chain disclosure and feasibility facts"
    #["ASP-RFC-10.05-RWCH-VERIFIED", "ASP-RFC-10.05-RWCH-AUTHORIZATION"] #[],
  declaration
    "ASPProof.SearchRouteRobustWindowedCalibrationHysteresis.authorized_robust_batch_rejects_exploration_mode"
    "exploration-mode-authorization-rejection"
    "an authorized exploitation batch cannot use exploration mode"
    #["ASP-RFC-10.05-RWCH-EXIT", "ASP-RFC-10.05-RWCH-AUTHORIZATION"] #[],
  declaration
    "ASPProof.SearchRouteRobustWindowedCalibrationHysteresis.authorized_robust_batch_has_zero_calibration_overhead"
    "robust-batch-zero-overhead-authorization"
    "authorized exploitation requires zero incremental calibration overhead"
    #["ASP-RFC-10.05-RWCH-OVERHEAD", "ASP-RFC-10.05-RWCH-AUTHORIZATION"]
    #[]
]

def receipt : Json :=
  .mkObj [
    ("axiomDependentDeclarationCount", .num 7),
    ("axiomFreeDeclarationCount", .num 13),
    ("axiomInventory", stringArray #["Quot.sound", "propext"]),
    ("declarationCount", .num 20),
    ("declarations", .arr declarations),
    ("hasSorryAx", .bool false),
    ("leanVersion", .str "4.32.2"),
    ("module", .str "ASPProof.SearchRouteRobustWindowedCalibrationHysteresis"),
    ("proofPackage", .str "ASPProof"),
    ("schemaId", .str "asp.lean-proof-audit.v1"),
    ("schemaVersion", .str "1"),
    ("sourcePath", .str "packages/proofs/ASPProof/SearchRouteRobustWindowedCalibrationHysteresis.lean")
  ]

end ASPProof.Audit.SearchRouteRobustWindowedCalibrationHysteresis
