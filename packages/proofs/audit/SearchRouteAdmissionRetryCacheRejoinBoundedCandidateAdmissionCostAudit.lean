import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinBoundedCandidateAdmissionCost

open ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinBoundedCandidateAdmissionCost

elab "writeSearchRouteAdmissionRetryCacheRejoinBoundedCandidateAdmissionCostAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-bounded-candidate-admission-cost-audit-v1.json"
    auditJson

writeSearchRouteAdmissionRetryCacheRejoinBoundedCandidateAdmissionCostAudit
