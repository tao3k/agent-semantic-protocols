import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinJointFence

open Lean Elab Command

elab "#writeSearchRouteAdmissionRetryCacheRejoinJointFenceAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-joint-fence-audit-v1.json"
    ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinJointFence.auditJson

#writeSearchRouteAdmissionRetryCacheRejoinJointFenceAudit
