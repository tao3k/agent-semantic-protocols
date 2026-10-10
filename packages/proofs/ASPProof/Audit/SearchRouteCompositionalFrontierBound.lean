-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteCompositionalFrontierBound

namespace ASPProof.Audit.SearchRouteCompositionalFrontierBound

def declarationFamilies : List (String × List String) :=
  [
    (
      "composition-soundness",
      [
        "and_unknown_sharing_max_is_sound",
        "and_disjoint_addition_is_sound",
        "identity_aware_and_bound_is_sound",
        "or_minimum_bound_is_sound"
      ]
    ),
    (
      "shared-witness-counterexample",
      [
        "naive_addition_is_not_a_lower_bound_for_shared_witness",
        "shared_witness_unknown_sharing_bound_is_exact",
        "shared_witness_identity_aware_bound_is_exact",
        "shared_witness_counterexample"
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
      ("rfc", .str "10.05.10.00.44"),
      (
        "module",
        .str "ASPProof.SearchRouteCompositionalFrontierBound"
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

end ASPProof.Audit.SearchRouteCompositionalFrontierBound
