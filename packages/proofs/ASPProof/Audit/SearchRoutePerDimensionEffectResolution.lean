-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRoutePerDimensionEffectResolution

namespace ASPProof.Audit.SearchRoutePerDimensionEffectResolution

def declarationFamilies : List (String × List String) :=
  [
    (
      "dimension-conservation",
      [
        "dimension_resolution_preserves_total",
        "vector_resolution_preserves_all_totals",
        "example_reservation_is_admissible"
      ]
    ),
    (
      "mixed-outcome",
      [
        "example_tokens_become_quarantined",
        "example_money_becomes_consumed",
        "example_provider_quota_becomes_consumed",
        "mixed_outcome_is_not_scalar",
        "resolved_dimensions_survive_one_unknown_dimension"
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
      ("rfc", .str "10.05.10.00.55"),
      (
        "module",
        .str "ASPProof.SearchRoutePerDimensionEffectResolution"
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

end ASPProof.Audit.SearchRoutePerDimensionEffectResolution
