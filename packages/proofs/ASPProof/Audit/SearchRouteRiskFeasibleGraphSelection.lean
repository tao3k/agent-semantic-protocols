-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import Lean
import ASPProof.SearchRouteRiskFeasibleGraphSelection

namespace ASPProof.Audit.SearchRouteRiskFeasibleGraphSelection

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
  free "admitted_candidate_is_route_and_risk_feasible" "composite-admission"
    "Composite admission projects route and risk feasibility"
    ["ASP-RFC-10.05-RFSG-FEASIBILITY-CONJUNCTION"],
  free "admitted_no_worse_left_is_risk_feasible" "admitted-comparison"
    "Admitted comparison always has a risk-feasible left candidate"
    ["ASP-RFC-10.05-RFSG-RISK-BEFORE-COST"],
  quot "safe_risk_envelope_is_valid" "risk-envelope-witness"
    "Concrete cap-five envelope satisfies expected, tail, round, and token gates"
    ["ASP-RFC-10.05-RFSG-FEASIBILITY-CONJUNCTION",
     "ASP-RFC-10.05-RFSG-NO-HOP-COMPENSATION"],
  prop "safe_candidate_risk_identity_matches" "identity-binding"
    "Concrete risk identity matches every envelope-owned field"
    ["ASP-RFC-10.05-RFSG-IDENTITY-PRODUCT"],
  quot "longer_candidate_is_risk_feasible" "risk-feasible-witness"
    "Concrete two-hop candidate is risk feasible"
    ["ASP-RFC-10.05-RFSG-FEASIBILITY-CONJUNCTION"],
  free "unsafe_cap_satisfies_round_bound_but_fails_token_bound"
    "hard-cap-counterexample"
    "Cap six passes round count but violates provider-token safety"
    ["ASP-RFC-10.05-RFSG-NO-HOP-COMPENSATION"],
  free "short_candidate_is_not_risk_feasible" "risk-infeasibility"
    "Concrete one-hop candidate is excluded by its hard token cap"
    ["ASP-RFC-10.05-RFSG-NO-HOP-COMPENSATION"],
  prop "short_candidate_is_route_feasible" "route-risk-independence"
    "Concrete short candidate satisfies route caps"
    ["ASP-RFC-10.05-RFSG-FEASIBILITY-CONJUNCTION"],
  prop "longer_candidate_is_route_feasible" "route-risk-independence"
    "Concrete longer candidate satisfies route caps"
    ["ASP-RFC-10.05-RFSG-FEASIBILITY-CONJUNCTION"],
  free "hop_first_prefers_short_risk_infeasible_candidate" "hop-first-counterexample"
    "Hop-first order prefers the risk-infeasible candidate"
    ["ASP-RFC-10.05-RFSG-RISK-BEFORE-COST"],
  quot "hop_first_preference_does_not_imply_risk_admission"
    "hop-admission-separation"
    "Hop-first and route feasibility do not imply risk admission"
    ["ASP-RFC-10.05-RFSG-NO-HOP-COMPENSATION",
     "ASP-RFC-10.05-RFSG-RISK-BEFORE-COST"],
  quot "longer_risk_feasible_route_is_admitted_over_shorter_route"
    "feasibility-first-selection"
    "Longer admitted route wins over shorter risk-infeasible route"
    ["ASP-RFC-10.05-RFSG-CONDITIONAL-SHORTEST",
     "ASP-RFC-10.05-RFSG-RISK-BEFORE-COST"],
  quot "model_prefix_cost_improvement_does_not_create_risk_admission"
    "cache-nonauthority"
    "Model prefix cost improvement cannot synthesize risk admission"
    ["ASP-RFC-10.05-RFSG-CACHE-NONAUTHORITY"],
  prop "legacy_route_identity_does_not_prove_calibration_comparability"
    "identity-product"
    "Legacy route equality does not hide calibration drift"
    ["ASP-RFC-10.05-RFSG-GENERATION-FENCE",
     "ASP-RFC-10.05-RFSG-IDENTITY-PRODUCT"],
  prop "legacy_route_identity_does_not_prove_cost_profile_comparability"
    "identity-product"
    "Legacy route equality does not hide search-cost drift"
    ["ASP-RFC-10.05-RFSG-GENERATION-FENCE",
     "ASP-RFC-10.05-RFSG-IDENTITY-PRODUCT"],
  free "selected_candidate_is_risk_feasible" "selection-soundness"
    "Every selected catalog member is risk feasible"
    ["ASP-RFC-10.05-RFSG-CONDITIONAL-SHORTEST",
     "ASP-RFC-10.05-RFSG-RISK-BEFORE-COST"],
  free "empty_admitted_catalog_has_no_selection" "empty-admission-fail-closed"
    "An empty admitted catalog cannot produce a selected route"
    ["ASP-RFC-10.05-RFSG-NO-LEAST-INFEASIBLE"]
]

def manifest : Lean.Json :=
  Lean.Json.mkObj [
    ("schemaId", Lean.toJson "asp.lean-proof-audit.v1"),
    ("schemaVersion", Lean.toJson "1"),
    ("leanVersion", Lean.toJson "4.32.2"),
    ("proofPackage", Lean.toJson "ASPProof"),
    ("module", Lean.toJson "ASPProof.SearchRouteRiskFeasibleGraphSelection"),
    ("sourcePath", Lean.toJson
      "packages/proofs/ASPProof/SearchRouteRiskFeasibleGraphSelection.lean"),
    ("declarationCount", Lean.toJson declarations.size),
    ("axiomFreeDeclarationCount", Lean.toJson 7),
    ("axiomDependentDeclarationCount", Lean.toJson 10),
    ("declarations", Lean.toJson declarations),
    ("axiomInventory", Lean.toJson ["Quot.sound", "propext"]),
    ("hasClassicalChoice", Lean.toJson false),
    ("hasNativeDecideAxiom", Lean.toJson false),
    ("hasSorryAx", Lean.toJson false),
    ("rfc", Lean.toJson "01.26-searchroute-risk-feasible-graph-selection"),
    ("status", Lean.toJson "kernel-compiled")
  ]

end ASPProof.Audit.SearchRouteRiskFeasibleGraphSelection
