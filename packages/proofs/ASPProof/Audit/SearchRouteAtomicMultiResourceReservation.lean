-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteAtomicMultiResourceReservation

namespace ASPProof.Audit.SearchRouteAtomicMultiResourceReservation

def declarationFamilies : List (String × List String) :=
  [
    (
      "atomic-admission",
      [
        "admitted_iff_all_dimensions_fit",
        "rejected_result_preserves_input_ledger",
        "successful_reservation_preserves_all_totals"
      ]
    ),
    (
      "partial-feasibility-counterexample",
      [
        "example_tokens_fit",
        "example_money_does_not_fit",
        "example_provider_quota_fits",
        "example_multi_resource_admission_is_rejected",
        "example_rejection_leaves_every_ledger_unchanged",
        "partial_component_feasibility_does_not_imply_admission"
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
      ("rfc", .str "10.05.10.00.54"),
      (
        "module",
        .str "ASPProof.SearchRouteAtomicMultiResourceReservation"
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

end ASPProof.Audit.SearchRouteAtomicMultiResourceReservation
