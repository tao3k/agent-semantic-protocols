import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinResilientAuthorityMedian

open ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinResilientAuthorityMedian

elab "writeSearchRouteAdmissionRetryCacheRejoinResilientAuthorityMedianAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-resilient-authority-median-audit-v1.json"
    auditJson

writeSearchRouteAdmissionRetryCacheRejoinResilientAuthorityMedianAudit
