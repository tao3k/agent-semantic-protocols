-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteDagResourceAlgebra

namespace ASPProof.Audit.SearchRouteDagResourceAlgebra

def declarationFamilies : List (String × List String) :=
  [
    (
      "reservation-accounting",
      [
        "reservation_preserves_token_accounting",
        "reservation_preserves_transition_accounting",
        "first_branch_can_reserve",
        "first_reservation_leaves_five_tokens",
        "second_branch_cannot_double_spend"
      ]
    ),
    (
      "composition-counterexample",
      [
        "branch_a_is_individually_feasible",
        "branch_b_is_individually_feasible",
        "sequential_plan_is_infeasible",
        "parallel_plan_is_infeasible",
        "individual_feasibility_does_not_compose",
        "parallel_and_sequential_span_differ"
      ]
    )
  ]

def expectedAxiomFree : List String :=
  declarationFamilies.flatMap Prod.snd

def familyJson (family : String × List String) : Lean.Json :=
  Lean.Json.mkObj
    [
      ("family", .str family.1),
      ("declarations", .arr (family.2.map Lean.Json.str).toArray)
    ]

def auditManifest : Lean.Json :=
  Lean.Json.mkObj
    [
      ("schema", .str "asp.lean-audit-manifest.v1"),
      ("rfc", .str "10.05.10.00.36"),
      (
        "module",
        .str "ASPProof.SearchRouteDagResourceAlgebra"
      ),
      (
        "families",
        .arr (declarationFamilies.map familyJson).toArray
      ),
      (
        "expectedAxiomFree",
        .arr (expectedAxiomFree.map Lean.Json.str).toArray
      )
    ]

end ASPProof.Audit.SearchRouteDagResourceAlgebra
