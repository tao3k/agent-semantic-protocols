import ASPProof.Audit.Receipt
import ASPProof.Audit.SearchRouteAdmissionRetryRootBarrier

open Lean Elab Command

elab "writeSearchRouteAdmissionRetryRootBarrierAudit" : command => do
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-root-barrier-audit-v1.json"
    ASPProof.Audit.SearchRouteAdmissionRetryRootBarrier.auditJson

writeSearchRouteAdmissionRetryRootBarrierAudit
