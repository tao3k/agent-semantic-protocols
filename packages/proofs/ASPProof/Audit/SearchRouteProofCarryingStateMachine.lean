-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteProofCarryingStateMachine

namespace ASPProof.Audit.SearchRouteProofCarryingStateMachine

def declarationFamilies : List (String × List String) :=
  [
    (
      "phase-safety",
      [
        "no_direct_exploring_to_executing",
        "no_direct_decision_to_executing",
        "execution_transition_requires_ready",
        "execution_transition_preserves_context",
        "execution_transition_preserves_completion_mode"
      ]
    ),
    (
      "bundle-binding",
      [
        "fresh_admission_is_execution_ready",
        "stale_ledger_admission_is_not_execution_ready"
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
      ("rfc", .str "10.05.10.00.46"),
      (
        "module",
        .str "ASPProof.SearchRouteProofCarryingStateMachine"
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

end ASPProof.Audit.SearchRouteProofCarryingStateMachine
