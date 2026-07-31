import ASPProof.Audit.Receipt
import ASPProof.Audit.SearchRouteAdmissionRetryCacheReplicaRejoin

open Lean Elab Command

elab "writeSearchRouteAdmissionRetryCacheReplicaRejoinAudit" : command => do
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-replica-rejoin-audit-v1.json"
    ASPProof.Audit.SearchRouteAdmissionRetryCacheReplicaRejoin.auditJson

writeSearchRouteAdmissionRetryCacheReplicaRejoinAudit
