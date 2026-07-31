import ASPProof.Audit.Core
import ASPProof.SearchRouteBoundedReconciliation

namespace ASPProof.Audit.SearchRouteBoundedReconciliation

def declarationFamilies : List (String × List String) :=
  [
    (
      "bounded-progress",
      [
        "reconciliation_self_loop_is_rejected",
        "exhausted_state_has_no_continuation",
        "run_length_le_initial_attempts",
        "run_length_le_initial_rounds",
        "run_length_le_initial_transitions"
      ]
    ),
    (
      "quarantine",
      [
        "exhausted_example_is_quarantined",
        "exhausted_example_cannot_release"
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
      ("rfc", .str "10.05.10.00.50"),
      (
        "module",
        .str "ASPProof.SearchRouteBoundedReconciliation"
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

end ASPProof.Audit.SearchRouteBoundedReconciliation

