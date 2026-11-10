-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteGenerationSafeBoundTransport

namespace ASPProof.Audit.SearchRouteGenerationSafeBoundTransport

def declarationFamilies : List (String × List String) :=
  [
    (
      "transport-soundness",
      [
        "bound_transport_is_sound",
        "monotone_cost_increase_preserves_lower_bound"
      ]
    ),
    (
      "generation-counterexample",
      [
        "old_problem_has_lower_bound_ten",
        "cheaper_alternative_invalidates_old_bound",
        "old_bound_does_not_imply_new_bound"
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
      ("rfc", .str "10.05.10.00.45"),
      (
        "module",
        .str "ASPProof.SearchRouteGenerationSafeBoundTransport"
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

end ASPProof.Audit.SearchRouteGenerationSafeBoundTransport
