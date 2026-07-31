import Lean
import ASPProof.SearchRouteAppendOnlyForecastCalibration

namespace ASPProof.Audit.SearchRouteAppendOnlyForecastCalibration

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
    "ASPProof.SearchRouteAppendOnlyForecastCalibration.exact_realization_has_zero_regret"
    "exact-realization-zero-regret"
    "an exact prediction and realization has zero coordinate and total regret"
    #["ASP-RFC-10.05-AOFC-OBSERVATION", "ASP-RFC-10.05-AOFC-REGRET"]
    #["Quot.sound", "propext"],
  declaration
    "ASPProof.SearchRouteAppendOnlyForecastCalibration.calibration_update_appends_exactly_one_observation"
    "append-only-single-observation"
    "a valid calibration update appends exactly the bound observation"
    #["ASP-RFC-10.05-AOFC-APPEND"] #[],
  declaration
    "ASPProof.SearchRouteAppendOnlyForecastCalibration.calibration_update_preserves_old_ledger_as_prefix"
    "append-only-prefix-preservation"
    "the prior observation ledger remains an ordered sublist of the updated ledger"
    #["ASP-RFC-10.05-AOFC-APPEND", "ASP-RFC-10.05-AOFC-HISTORY"] #[],
  declaration
    "ASPProof.SearchRouteAppendOnlyForecastCalibration.calibration_update_increments_sample_count"
    "sample-count-advance"
    "every valid update advances sample count by exactly one"
    #["ASP-RFC-10.05-AOFC-UPDATE"] #[],
  declaration
    "ASPProof.SearchRouteAppendOnlyForecastCalibration.calibration_update_accumulates_exact_regret"
    "exact-regret-accumulation"
    "cumulative regret advances by exactly the appended observation regret"
    #["ASP-RFC-10.05-AOFC-REGRET", "ASP-RFC-10.05-AOFC-UPDATE"] #[],
  declaration
    "ASPProof.SearchRouteAppendOnlyForecastCalibration.nonbreach_resets_consecutive_breaches"
    "nonbreach-counter-reset"
    "a regret value within threshold resets the consecutive-breach counter"
    #["ASP-RFC-10.05-AOFC-BREACH"] #[],
  declaration
    "ASPProof.SearchRouteAppendOnlyForecastCalibration.breach_increments_consecutive_breaches"
    "breach-counter-increment"
    "a regret value above threshold increments the consecutive-breach counter"
    #["ASP-RFC-10.05-AOFC-BREACH"] #[],
  declaration
    "ASPProof.SearchRouteAppendOnlyForecastCalibration.persistent_breach_requires_exploration"
    "persistent-breach-exploration"
    "breach persistence at the configured limit requires exploration mode"
    #["ASP-RFC-10.05-AOFC-EXPLORE"] #["propext"],
  declaration
    "ASPProof.SearchRouteAppendOnlyForecastCalibration.bounded_bias_requires_degraded_mode"
    "bounded-bias-degraded-mode"
    "positive breach history below the exploration limit requires degraded mode"
    #["ASP-RFC-10.05-AOFC-DEGRADED"] #["propext"],
  declaration
    "ASPProof.SearchRouteAppendOnlyForecastCalibration.zero_bias_with_positive_limit_requires_normal_mode"
    "zero-bias-normal-mode"
    "zero consecutive breaches with a positive limit requires normal mode"
    #["ASP-RFC-10.05-AOFC-DEGRADED"] #["propext"],
  declaration
    "ASPProof.SearchRouteAppendOnlyForecastCalibration.calibration_update_preserves_old_decision_binding"
    "historical-decision-binding-preservation"
    "a calibration update does not change an existing decision snapshot binding"
    #["ASP-RFC-10.05-AOFC-HISTORY"] #[],
  declaration
    "ASPProof.SearchRouteAppendOnlyForecastCalibration.new_snapshot_cannot_retroactively_rebind_old_decision"
    "retroactive-rebinding-rejection"
    "a newly versioned snapshot cannot retroactively bind an old decision"
    #["ASP-RFC-10.05-AOFC-SNAPSHOT", "ASP-RFC-10.05-AOFC-HISTORY"]
    #["Quot.sound", "propext"],
  declaration
    "ASPProof.SearchRouteAppendOnlyForecastCalibration.exploration_mode_blocks_exploitation"
    "exploration-exploitation-exclusion"
    "exploration mode makes the calibration state ineligible for exploitation"
    #["ASP-RFC-10.05-AOFC-EXPLORE", "ASP-RFC-10.05-AOFC-AUTHORIZATION"] #[],
  declaration
    "ASPProof.SearchRouteAppendOnlyForecastCalibration.authorized_future_batch_requires_verified_chain"
    "future-authorization-chain-proof"
    "future batch authorization requires verified transition-chain evidence"
    #["ASP-RFC-10.05-AOFC-AUTHORIZATION", "ASP-RFC-10.05-AOFC-ROUTER"] #[],
  declaration
    "ASPProof.SearchRouteAppendOnlyForecastCalibration.authorized_future_batch_requires_valid_disclosure"
    "future-authorization-disclosure-proof"
    "future batch authorization requires valid selective disclosure"
    #["ASP-RFC-10.05-AOFC-AUTHORIZATION"] #[],
  declaration
    "ASPProof.SearchRouteAppendOnlyForecastCalibration.authorized_future_batch_requires_feasible_batch"
    "future-authorization-feasibility"
    "future batch authorization requires a feasible forecast-bound batch"
    #["ASP-RFC-10.05-AOFC-AUTHORIZATION"] #[],
  declaration
    "ASPProof.SearchRouteAppendOnlyForecastCalibration.authorized_future_batch_requires_exploitation_eligibility"
    "future-authorization-calibration-eligibility"
    "future batch authorization requires calibration eligibility for exploitation"
    #["ASP-RFC-10.05-AOFC-AUTHORIZATION", "ASP-RFC-10.05-AOFC-ROUTER"] #[]
]

def receipt : Json :=
  .mkObj [
    ("axiomDependentDeclarationCount", .num 5),
    ("axiomFreeDeclarationCount", .num 12),
    ("axiomInventory", stringArray #["Quot.sound", "propext"]),
    ("declarationCount", .num 17),
    ("declarations", .arr declarations),
    ("hasSorryAx", .bool false),
    ("leanVersion", .str "4.32.2"),
    ("module", .str "ASPProof.SearchRouteAppendOnlyForecastCalibration"),
    ("proofPackage", .str "ASPProof"),
    ("schemaId", .str "asp.lean-proof-audit.v1"),
    ("schemaVersion", .str "1"),
    ("sourcePath", .str "packages/proofs/ASPProof/SearchRouteAppendOnlyForecastCalibration.lean")
  ]

end ASPProof.Audit.SearchRouteAppendOnlyForecastCalibration
