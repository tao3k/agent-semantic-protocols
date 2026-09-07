-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteCertifiedAmbiguity

namespace ASPProof.Audit.SearchRouteCertifiedAmbiguity

def declarationFamilies : List (String × List String) :=
  [
    (
      "rank-soundness",
      [
        "empty_complete_certificate_implies_decision_sufficient",
        "status_certificate_proves_decision_sufficiency",
        "tiny_certificate_is_sound",
        "tiny_certificate_is_complete",
        "status_certificate_is_sound",
        "status_certificate_is_complete"
      ]
    ),
    (
      "certified-example",
      [
        "tiny_projection_is_not_decision_sufficient",
        "tiny_certificate_has_ambiguity_two",
        "status_certificate_has_ambiguity_zero",
        "status_is_strict_refinement_of_tiny",
        "scope_drift_rejects_alleged_refinement",
        "generation_drift_rejects_alleged_refinement"
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
      ("rfc", .str "10.05.10.00.42"),
      (
        "module",
        .str "ASPProof.SearchRouteCertifiedAmbiguity"
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

end ASPProof.Audit.SearchRouteCertifiedAmbiguity
