import ASPProof.Audit.Core
import ASPProof.SearchRouteAdmissionRetryCacheRejoinAccountingReceiptIdentity

namespace ASPProof.Audit

def writeReceipt
    (path : System.FilePath)
    (auditJson : Lean.Elab.TermElabM Lean.Json) :
    Lean.Elab.Command.CommandElabM Unit := do
  let audit ← Lean.Elab.Command.liftTermElabM auditJson
  IO.FS.writeFile path (audit.pretty ++ "\n")
  Lean.logInfo m!"wrote {path}"

end ASPProof.Audit

namespace ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinAccountingReceiptIdentity

open ASPProof.Audit.Core
open ASPProof.SearchRouteAdmissionRetryCacheRejoinAccountingReceiptIdentity

def targets : List Target := [
  Target.mk
    ``swapping_certificates_preserves_accounting_receipt_identity
    "unordered-certificate-pair-symmetry"
    ["CARI-PAIR", "CARI-ACCOUNTING"],
  Target.mk
    ``equal_accounting_receipt_implies_same_semantic_owners
    "accounting-owner-reflection"
    ["CARI-ACCOUNTING", "CARI-COLLISION"],
  Target.mk
    ``changed_membership_prevents_accounting_receipt_reuse
    "membership-change-invalidation"
    ["CARI-INVALIDATE", "CARI-ACCOUNTING"],
  Target.mk
    ``changed_unordered_pair_prevents_accounting_receipt_reuse
    "certificate-pair-change-invalidation"
    ["CARI-INVALIDATE", "CARI-PAIR"],
  Target.mk
    ``changed_projection_prevents_accounting_receipt_reuse
    "projection-change-invalidation"
    ["CARI-INVALIDATE", "CARI-PROJECTION"],
  Target.mk
    ``fault_bound_snapshot_change_preserves_accounting_but_changes_verdict
    "two-tier-fault-bound-invalidation"
    ["CARI-VERDICT", "CARI-CACHE"],
  Target.mk
    ``equal_verdict_implies_same_accounting_owners_and_fault_bound
    "verdict-owner-reflection"
    ["CARI-VERDICT", "CARI-COLLISION"]
]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryCacheRejoinAccountingReceiptIdentity"
    "ASPProof/SearchRouteAdmissionRetryCacheRejoinAccountingReceiptIdentity.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinAccountingReceiptIdentity
