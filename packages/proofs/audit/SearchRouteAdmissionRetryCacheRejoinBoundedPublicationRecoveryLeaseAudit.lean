import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinBoundedPublicationRecoveryLease

open ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinBoundedPublicationRecoveryLease

elab "writeSearchRouteAdmissionRetryCacheRejoinBoundedPublicationRecoveryLeaseAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-bounded-publication-recovery-lease-audit-v1.json"
    auditJson

writeSearchRouteAdmissionRetryCacheRejoinBoundedPublicationRecoveryLeaseAudit
