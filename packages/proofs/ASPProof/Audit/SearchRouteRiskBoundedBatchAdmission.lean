-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import Lean
import ASPProof.SearchRouteRiskBoundedBatchAdmission

namespace ASPProof.Audit.SearchRouteRiskBoundedBatchAdmission

private def canonicalStrings (values : List String) : List String :=
  (values.mergeSort (fun left right => decide (left < right))).eraseDups

private def declaration
    (name theoremFamily type : String)
    (rfcClauseIds axioms : List String) : Lean.Json :=
  Lean.Json.mkObj [
    ("name", Lean.toJson name),
    ("kind", Lean.toJson "theorem"),
    ("theoremFamily", Lean.toJson theoremFamily),
    ("type", Lean.toJson type),
    ("rfcClauseIds", Lean.toJson (canonicalStrings rfcClauseIds)),
    ("axioms", Lean.toJson (canonicalStrings axioms)),
    ("hasSorryAx", Lean.toJson false)
  ]

private def free (name family type : String) (clauses : List String) : Lean.Json :=
  declaration name family type clauses []

private def prop (name family type : String) (clauses : List String) : Lean.Json :=
  declaration name family type clauses ["propext"]

private def quot (name family type : String) (clauses : List String) : Lean.Json :=
  declaration name family type clauses ["Quot.sound", "propext"]

def declarations : Array Lean.Json := #[
  quot "event_failure_numerator_le_fallback_mass" "claim-weighted-risk"
    "Nonempty batch event risk is bounded by fallback claim mass"
    ["ASP-RFC-10.05-RBBA-CLAIM-WEIGHTED-EXPECTATION",
     "ASP-RFC-10.05-RBBA-EXACT-PARTITION-MASS"],
  free "singleton_multi_claim_event_risk_is_strict" "claim-weighted-risk"
    "A positive-risk multi-claim batch has strictly larger fallback mass"
    ["ASP-RFC-10.05-RBBA-CLAIM-WEIGHTED-EXPECTATION"],
  prop "fallback_within_hard_cap_is_round_safe" "hard-cap-safety"
    "Realized fallback within the cap preserves the RFC 01.24 round bound"
    ["ASP-RFC-10.05-RBBA-HARD-FALLBACK-CAP"],
  quot "fallback_within_hard_cap_is_token_safe" "hard-cap-safety"
    "Realized fallback within the cap preserves the RFC 01.24 token bound"
    ["ASP-RFC-10.05-RBBA-HARD-FALLBACK-CAP"],
  free "valid_tail_certificate_targets_hard_cap" "tail-cap-binding"
    "Tail certificate threshold equals the enforced hard fallback cap"
    ["ASP-RFC-10.05-RBBA-HARD-FALLBACK-CAP",
     "ASP-RFC-10.05-RBBA-TAIL-SEPARATION"],
  free "valid_envelope_reports_claim_weighted_mass" "envelope-integrity"
    "Valid envelope reports the exact claim-weighted mass"
    ["ASP-RFC-10.05-RBBA-CLAIM-WEIGHTED-EXPECTATION",
     "ASP-RFC-10.05-RBBA-EXACT-PARTITION-MASS"],
  prop "valid_envelope_hard_cap_preserves_realized_round_bound" "envelope-safety"
    "Valid envelope cap transports to realized round safety"
    ["ASP-RFC-10.05-RBBA-HARD-FALLBACK-CAP",
     "ASP-RFC-10.05-RBBA-NO-EXPECTED-AUTHORITY"],
  quot "valid_envelope_hard_cap_preserves_realized_token_bound" "envelope-safety"
    "Valid envelope cap transports to realized token safety"
    ["ASP-RFC-10.05-RBBA-HARD-FALLBACK-CAP",
     "ASP-RFC-10.05-RBBA-NO-EXPECTED-AUTHORITY"],
  free "example_event_count_understates_fallback_mass" "risk-counterexample"
    "Concrete event count 25 understates fallback mass 100"
    ["ASP-RFC-10.05-RBBA-CLAIM-WEIGHTED-EXPECTATION"],
  prop "example_expected_cost_is_admissible" "expected-cost-example"
    "Concrete plan passes expected round and token admission"
    ["ASP-RFC-10.05-RBBA-EXPECTED-ROUND-ADMISSION",
     "ASP-RFC-10.05-RBBA-EXPECTED-TOKEN-ADMISSION"],
  prop "expected_admission_does_not_imply_realized_round_safety"
    "expectation-hard-bound-separation"
    "Expected-cost admission coexists with unsafe realized full fallback"
    ["ASP-RFC-10.05-RBBA-NO-EXPECTED-AUTHORITY"],
  quot "example_hard_cap_closes_realized_round_bound" "hard-cap-example"
    "Concrete hard cap closes the realized round bound"
    ["ASP-RFC-10.05-RBBA-HARD-FALLBACK-CAP"],
  quot "example_hard_cap_closes_realized_token_bound" "hard-cap-example"
    "Concrete hard cap closes the realized token bound"
    ["ASP-RFC-10.05-RBBA-HARD-FALLBACK-CAP"],
  prop "zero_risk_singleton_fragmentation_has_no_expected_round_savings"
    "fragmentation-cost"
    "Zero-risk singleton fragmentation equals the individual round baseline"
    ["ASP-RFC-10.05-RBBA-EXPECTED-ROUND-ADMISSION"],
  prop "semantic_rejection_cannot_form_risk_cell" "risk-eligibility"
    "Semantic rejection cannot enter fallback risk mass"
    ["ASP-RFC-10.05-RBBA-ELIGIBLE-RISK-ONLY"],
  prop "agent_preference_cannot_form_risk_cell" "risk-eligibility"
    "Agent preference cannot enter fallback risk mass"
    ["ASP-RFC-10.05-RBBA-ELIGIBLE-RISK-ONLY"],
  prop "plan_only_hit_does_not_prove_risk_cache_reuse" "risk-cache-identity"
    "Plan equality does not prove calibration-bound risk reuse"
    ["ASP-RFC-10.05-RBBA-CALIBRATION-IDENTITY"],
  prop "cost_profile_change_invalidates_risk_cache_reuse" "risk-cache-identity"
    "Changing search token costs invalidates risk-cache reuse"
    ["ASP-RFC-10.05-RBBA-CALIBRATION-IDENTITY",
     "ASP-RFC-10.05-RBBA-CACHE-SEPARATION"],
  prop "model_prefix_hit_does_not_prove_risk_cache_reuse" "cache-separation"
    "Model prefix-cache equality does not prove search-risk reuse"
    ["ASP-RFC-10.05-RBBA-CACHE-SEPARATION"]
]

def manifest : Lean.Json :=
  Lean.Json.mkObj [
    ("schemaId", Lean.toJson "asp.lean-proof-audit.v1"),
    ("schemaVersion", Lean.toJson "1"),
    ("leanVersion", Lean.toJson "4.32.2"),
    ("proofPackage", Lean.toJson "ASPProof"),
    ("module", Lean.toJson "ASPProof.SearchRouteRiskBoundedBatchAdmission"),
    ("sourcePath", Lean.toJson
      "packages/proofs/ASPProof/SearchRouteRiskBoundedBatchAdmission.lean"),
    ("declarationCount", Lean.toJson declarations.size),
    ("axiomFreeDeclarationCount", Lean.toJson 4),
    ("axiomDependentDeclarationCount", Lean.toJson 15),
    ("declarations", Lean.toJson declarations),
    ("axiomInventory", Lean.toJson ["Quot.sound", "propext"]),
    ("hasClassicalChoice", Lean.toJson false),
    ("hasNativeDecideAxiom", Lean.toJson false),
    ("hasSorryAx", Lean.toJson false),
    ("rfc", Lean.toJson "01.25-searchroute-risk-bounded-batch-admission"),
    ("status", Lean.toJson "kernel-compiled")
  ]

end ASPProof.Audit.SearchRouteRiskBoundedBatchAdmission
