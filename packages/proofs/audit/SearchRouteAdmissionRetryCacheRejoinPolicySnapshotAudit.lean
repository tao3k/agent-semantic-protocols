import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinPolicySnapshot

open Lean Elab Command

elab "#writeSearchRouteAdmissionRetryCacheRejoinPolicySnapshotAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-policy-snapshot-audit-v1.json"
    ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinPolicySnapshot.auditJson

#writeSearchRouteAdmissionRetryCacheRejoinPolicySnapshotAudit
