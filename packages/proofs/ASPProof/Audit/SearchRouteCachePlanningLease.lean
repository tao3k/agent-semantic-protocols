-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteCachePlanningLease

namespace ASPProof.Audit.SearchRouteCachePlanningLease

def declarationFamilies : List (String × List String) :=
  [
    (
      "planning-authority",
      [
        "conservative_is_planning_admissible",
        "post_hoc_is_not_planning_admissible"
      ]
    ),
    (
      "lease-drift",
      [
        "prefix_key_drift_rejects_lease",
        "runtime_generation_drift_rejects_lease",
        "request_drift_rejects_lease",
        "expired_lease_is_rejected",
        "prefix_state_drift_rejects_lease"
      ]
    ),
    (
      "bound-example",
      [
        "example_hit_lease_is_planning_admissible",
        "example_post_hoc_hit_is_accounting_admissible"
      ]
    ),
    (
      "causal-counterexample",
      [
        "post_hoc_route_a_planning_cost_is_conservative",
        "post_hoc_route_a_accounting_cost_is_observed_hit",
        "post_hoc_observation_does_not_lower_prior_planning_cost"
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
      ("rfc", .str "10.05.10.00.34"),
      (
        "module",
        .str "ASPProof.SearchRouteCachePlanningLease"
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

end ASPProof.Audit.SearchRouteCachePlanningLease
