import ASPProof.Audit.Core
import ASPProof.SearchRouteCrashSafeReconciliation

namespace ASPProof.Audit.SearchRouteCrashSafeReconciliation

def declarationFamilies : List (String × List String) :=
  [
    (
      "ambiguous-outcome",
      [
        "ambiguous_execution_permits_reconciliation",
        "ambiguous_execution_rejects_execute",
        "ambiguous_execution_rejects_accept_completed",
        "identity_mismatch_rejects_every_action"
      ]
    ),
    (
      "durable-resume",
      [
        "admitted_not_started_permits_execute",
        "completed_checkpoint_permits_stay_completed",
        "completed_checkpoint_rejects_execute",
        "completed_checkpoint_rejects_reconciliation",
        "completed_checkpoint_rejects_accept_completed"
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
      ("rfc", .str "10.05.10.00.49"),
      (
        "module",
        .str "ASPProof.SearchRouteCrashSafeReconciliation"
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

end ASPProof.Audit.SearchRouteCrashSafeReconciliation
