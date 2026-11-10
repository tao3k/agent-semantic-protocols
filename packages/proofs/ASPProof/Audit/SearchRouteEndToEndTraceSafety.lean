-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteEndToEndTraceSafety

namespace ASPProof.Audit.SearchRouteEndToEndTraceSafety

def declarationFamilies : List (String × List String) :=
  [
    (
      "execution-trace",
      [
        "exploration_run_preserves_context",
        "execution_trace_has_resolved_decision",
        "execution_trace_has_ready_admission",
        "execution_trace_preserves_context",
        "execution_trace_end_to_end_safety",
        "cross_context_trace_is_impossible"
      ]
    ),
    (
      "completion-trace",
      [
        "completion_trace_preserves_context",
        "completion_trace_preserves_mode",
        "completion_trace_preserves_request"
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
      ("rfc", .str "10.05.10.00.48"),
      (
        "module",
        .str "ASPProof.SearchRouteEndToEndTraceSafety"
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

end ASPProof.Audit.SearchRouteEndToEndTraceSafety
