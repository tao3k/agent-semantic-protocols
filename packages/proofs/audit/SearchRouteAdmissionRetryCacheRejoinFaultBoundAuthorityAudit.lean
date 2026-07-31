import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinFaultBoundAuthority

open ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinFaultBoundAuthority

elab "writeSearchRouteAdmissionRetryCacheRejoinFaultBoundAuthorityAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-fault-bound-authority-audit-v1.json"
    auditJson

writeSearchRouteAdmissionRetryCacheRejoinFaultBoundAuthorityAudit
