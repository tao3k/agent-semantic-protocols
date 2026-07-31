import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinWeightedAuthorityQuantile

open ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinWeightedAuthorityQuantile

elab "writeSearchRouteAdmissionRetryCacheRejoinWeightedAuthorityQuantileAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-weighted-authority-quantile-audit-v1.json"
    auditJson

writeSearchRouteAdmissionRetryCacheRejoinWeightedAuthorityQuantileAudit
