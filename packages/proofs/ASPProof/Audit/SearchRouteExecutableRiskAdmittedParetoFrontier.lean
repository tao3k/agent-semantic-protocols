-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import Lean
import ASPProof.SearchRouteExecutableRiskAdmittedParetoFrontier

namespace ASPProof.Audit.SearchRouteExecutableRiskAdmittedParetoFrontier

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

def declarations : Array Lean.Json := #[
  free "certified_member_is_admitted" "frontier-soundness"
    "Every certified frontier member is admitted before cost comparison"
    ["ASP-RFC-10.05-ERAPF-ADMISSION-BEFORE-DOMINANCE"],
  free "certified_member_is_catalogued" "frontier-membership"
    "Every certified frontier member belongs to the covered catalog"
    ["ASP-RFC-10.05-ERAPF-FRONTIER-COVERAGE"],
  free "omitted_admitted_candidate_has_retained_dominator" "frontier-coverage"
    "Every omitted admitted catalog candidate has a retained strict dominator"
    ["ASP-RFC-10.05-ERAPF-FRONTIER-COVERAGE"],
  free "non_equivalent_candidates_do_not_dominate" "completion-partition"
    "Dominance cannot cross a completion-equivalence class"
    ["ASP-RFC-10.05-ERAPF-COMPLETION-PARTITION"],
  free "inadmissible_candidate_does_not_dominate" "admission-nonauthority"
    "An inadmissible candidate cannot evict an admitted frontier candidate"
    ["ASP-RFC-10.05-ERAPF-ADMISSION-BEFORE-DOMINANCE"],
  free "reverse_no_worse_blocks_strict_dominance" "strict-dominance"
    "Reverse no-worse evidence refutes a strict dominance claim"
    ["ASP-RFC-10.05-ERAPF-EQUAL-COST-NONINTERCHANGEABILITY"],
  free "equal_cost_candidates_do_not_strictly_dominate" "equal-cost-identity"
    "Mutually no-worse costs do not justify dropping a distinct route identity"
    ["ASP-RFC-10.05-ERAPF-EQUAL-COST-NONINTERCHANGEABILITY"],
  free "certified_frontier_is_empty_when_admission_is_empty" "empty-admission"
    "An empty admitted domain cannot produce a certified frontier member"
    ["ASP-RFC-10.05-ERAPF-EMPTY-FAIL-CLOSED"],
  free "retained_candidates_do_not_strictly_dominate_each_other" "frontier-antichain"
    "Distinct retained candidates form a strict-dominance antichain"
    ["ASP-RFC-10.05-ERAPF-FRONTIER-ANTICHAIN"]
]

def manifest : Lean.Json :=
  Lean.Json.mkObj [
    ("schemaId", Lean.toJson "asp.lean-proof-audit.v1"),
    ("schemaVersion", Lean.toJson "1"),
    ("leanVersion", Lean.toJson "4.32.2"),
    ("proofPackage", Lean.toJson "ASPProof"),
    ("module", Lean.toJson
      "ASPProof.SearchRouteExecutableRiskAdmittedParetoFrontier"),
    ("sourcePath", Lean.toJson
      "packages/proofs/ASPProof/SearchRouteExecutableRiskAdmittedParetoFrontier.lean"),
    ("declarationCount", Lean.toJson declarations.size),
    ("axiomFreeDeclarationCount", Lean.toJson 9),
    ("axiomDependentDeclarationCount", Lean.toJson 0),
    ("declarations", Lean.toJson declarations),
    ("axiomInventory", Lean.toJson ([] : List String)),
    ("hasClassicalChoice", Lean.toJson false),
    ("hasNativeDecideAxiom", Lean.toJson false),
    ("hasSorryAx", Lean.toJson false),
    ("rfc", Lean.toJson
      "01.27-searchroute-executable-risk-admitted-pareto-frontier"),
    ("status", Lean.toJson "kernel-compiled")
  ]

end ASPProof.Audit.SearchRouteExecutableRiskAdmittedParetoFrontier
