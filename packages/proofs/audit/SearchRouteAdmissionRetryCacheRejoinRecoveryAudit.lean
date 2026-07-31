import ASPProof.Audit.Receipt
import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinRecovery

open Lean Elab Command

elab "writeSearchRouteAdmissionRetryCacheRejoinRecoveryAudit" : command => do
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-recovery-audit-v1.json"
    ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinRecovery.auditJson

writeSearchRouteAdmissionRetryCacheRejoinRecoveryAudit
