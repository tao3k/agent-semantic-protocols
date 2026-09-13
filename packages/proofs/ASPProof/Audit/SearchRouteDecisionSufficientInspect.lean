-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteDecisionSufficientInspect

namespace ASPProof.Audit.SearchRouteDecisionSufficientInspect

def declarationFamilies : List (String × List String) :=
  [
    (
      "decision-sufficiency",
      [
        "no_tiny_decoder_is_decision_correct",
        "status_projection_is_decision_correct",
        "full_state_projection_is_decision_correct"
      ]
    ),
    (
      "compactness-counterexample",
      [
        "status_projection_is_cheaper_than_full_state",
        "misleading_tiny_surface_is_cheaper",
        "misleading_tiny_surface_is_not_decision_correct",
        "compactness_does_not_imply_decision_sufficiency"
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
      ("rfc", .str "10.05.10.00.40"),
      (
        "module",
        .str "ASPProof.SearchRouteDecisionSufficientInspect"
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

end ASPProof.Audit.SearchRouteDecisionSufficientInspect
