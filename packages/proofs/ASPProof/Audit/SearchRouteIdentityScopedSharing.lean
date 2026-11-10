-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteIdentityScopedSharing

namespace ASPProof.Audit.SearchRouteIdentityScopedSharing

def declarationFamilies : List (String × List String) :=
  [
    (
      "identity-sharing",
      [
        "shareable_iff_complete_identity_equal",
        "repeated_identity_tree_costs_twenty",
        "repeated_identity_dag_costs_ten",
        "equal_atom_label_does_not_imply_shareable",
        "generation_drift_prevents_deduplication"
      ]
    ),
    (
      "cache-domain-separation",
      [
        "distinct_requests_can_share_semantic_identity",
        "shared_evidence_does_not_dedup_request_prompt_cost",
        "semantic_and_prompt_charges_remain_separate"
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
      ("rfc", .str "10.05.10.00.38"),
      (
        "module",
        .str "ASPProof.SearchRouteIdentityScopedSharing"
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

end ASPProof.Audit.SearchRouteIdentityScopedSharing
