-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import Lean
import ASPProof.SearchRouteForecastBoundParetoBatchScheduling

namespace ASPProof.Audit.SearchRouteForecastBoundParetoBatchScheduling

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
    "ASPProof.SearchRouteForecastBoundParetoBatchScheduling.valid_forecast_bounds_every_probability"
    "forecast-probability-bounds"
    "all cache-hit forecasts are bounded basis-point values"
    #["ASP-RFC-10.05-FPBS-FORECAST"] #[],
  declaration
    "ASPProof.SearchRouteForecastBoundParetoBatchScheduling.estimated_cache_work_is_lane_scoped"
    "lane-scoped-work-estimation"
    "each cache forecast affects only its corresponding estimated work lane"
    #["ASP-RFC-10.05-FPBS-ESTIMATE"] #[],
  declaration
    "ASPProof.SearchRouteForecastBoundParetoBatchScheduling.feasible_batch_is_nonempty"
    "feasible-batch-nonempty"
    "every feasible inspect batch is non-empty"
    #["ASP-RFC-10.05-FPBS-FEASIBLE"] #[],
  declaration
    "ASPProof.SearchRouteForecastBoundParetoBatchScheduling.feasible_batch_is_ordered_sublist"
    "feasible-batch-ordered-sublist"
    "feasible batch preserves remaining-edge order"
    #["ASP-RFC-10.05-FPBS-FEASIBLE"] #[],
  declaration
    "ASPProof.SearchRouteForecastBoundParetoBatchScheduling.feasible_batch_makes_strict_progress"
    "feasible-batch-strict-progress"
    "feasible non-empty batch reports positive progress"
    #["ASP-RFC-10.05-FPBS-FEASIBLE"] #["propext"],
  declaration
    "ASPProof.SearchRouteForecastBoundParetoBatchScheduling.forecast_mismatch_rejects_dominance"
    "forecast-mismatch-dominance-rejection"
    "plans from different cache forecasts cannot dominate each other"
    #["ASP-RFC-10.05-FPBS-IDENTITY"] #[],
  declaration
    "ASPProof.SearchRouteForecastBoundParetoBatchScheduling.dominance_is_irreflexive"
    "pareto-dominance-irreflexivity"
    "no batch plan dominates itself"
    #["ASP-RFC-10.05-FPBS-DOMINANCE"] #["Quot.sound", "propext"],
  declaration
    "ASPProof.SearchRouteForecastBoundParetoBatchScheduling.pareto_optimal_plan_is_not_dominated"
    "pareto-frontier-safety"
    "feasible candidate cannot dominate a Pareto-optimal chosen plan"
    #["ASP-RFC-10.05-FPBS-PARETO", "ASP-RFC-10.05-FPBS-ROUTER"] #[],
  declaration
    "ASPProof.SearchRouteForecastBoundParetoBatchScheduling.dominated_candidate_is_not_pareto_optimal"
    "dominated-plan-rejection"
    "a candidate dominated by a feasible member is not Pareto-optimal"
    #["ASP-RFC-10.05-FPBS-PARETO"] #[],
  declaration
    "ASPProof.SearchRouteForecastBoundParetoBatchScheduling.lower_token_cost_alone_does_not_establish_dominance"
    "token-only-dominance-rejection"
    "lower token cost alone does not establish Pareto dominance"
    #["ASP-RFC-10.05-FPBS-TRADEOFF"] #["Quot.sound", "propext"],
  declaration
    "ASPProof.SearchRouteForecastBoundParetoBatchScheduling.token_value_tradeoff_is_pareto_incomparable"
    "token-value-pareto-incomparability"
    "lower-token lower-value and higher-token higher-value plans are incomparable"
    #["ASP-RFC-10.05-FPBS-TRADEOFF"] #["Quot.sound", "propext"],
  declaration
    "ASPProof.SearchRouteForecastBoundParetoBatchScheduling.equal_forecast_digest_does_not_establish_equal_forecast"
    "forecast-digest-only-counterexample"
    "equal forecast digest alone does not establish equal finite forecast data"
    #["ASP-RFC-10.05-FPBS-IDENTITY"] #[],
  declaration
    "ASPProof.SearchRouteForecastBoundParetoBatchScheduling.realization_requires_forecast_digest_binding"
    "realization-forecast-binding"
    "realized receipt binds the immutable pre-execution forecast digest"
    #["ASP-RFC-10.05-FPBS-REALIZATION"] #[],
  declaration
    "ASPProof.SearchRouteForecastBoundParetoBatchScheduling.one_forecast_allows_different_realized_cache_hits"
    "forecast-realization-nondeterminism"
    "one forecast may produce different realized cache-hit outcomes"
    #["ASP-RFC-10.05-FPBS-REALIZATION"] #[],
  declaration
    "ASPProof.SearchRouteForecastBoundParetoBatchScheduling.authorized_batch_requires_verified_chain"
    "batch-authorization-chain-proof"
    "authorized batch requires verified transition-chain evidence"
    #["ASP-RFC-10.05-FPBS-AUTHORIZATION"] #[],
  declaration
    "ASPProof.SearchRouteForecastBoundParetoBatchScheduling.authorized_batch_requires_valid_disclosure"
    "batch-authorization-disclosure-proof"
    "authorized batch requires valid selective disclosure"
    #["ASP-RFC-10.05-FPBS-AUTHORIZATION"] #[],
  declaration
    "ASPProof.SearchRouteForecastBoundParetoBatchScheduling.authorized_batch_requires_feasibility"
    "batch-authorization-feasibility"
    "authorized batch requires strict feasible progress"
    #["ASP-RFC-10.05-FPBS-AUTHORIZATION"] #[]
]

def receipt : Json :=
  .mkObj [
    ("axiomDependentDeclarationCount", .num 4),
    ("axiomFreeDeclarationCount", .num 13),
    ("axiomInventory", stringArray #["Quot.sound", "propext"]),
    ("declarationCount", .num 17),
    ("declarations", .arr declarations),
    ("hasSorryAx", .bool false),
    ("leanVersion", .str "4.32.2"),
    ("module", .str "ASPProof.SearchRouteForecastBoundParetoBatchScheduling"),
    ("proofPackage", .str "ASPProof"),
    ("schemaId", .str "asp.lean-proof-audit.v1"),
    ("schemaVersion", .str "1"),
    ("sourcePath", .str "packages/proofs/ASPProof/SearchRouteForecastBoundParetoBatchScheduling.lean")
  ]

end ASPProof.Audit.SearchRouteForecastBoundParetoBatchScheduling
