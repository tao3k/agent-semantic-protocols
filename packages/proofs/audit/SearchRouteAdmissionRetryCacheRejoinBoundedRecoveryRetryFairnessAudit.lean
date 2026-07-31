import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinBoundedRecoveryRetryFairness

open ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinBoundedRecoveryRetryFairness

elab "writeSearchRouteAdmissionRetryCacheRejoinBoundedRecoveryRetryFairnessAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-bounded-recovery-retry-fairness-audit-v1.json"
    auditJson

writeSearchRouteAdmissionRetryCacheRejoinBoundedRecoveryRetryFairnessAudit
