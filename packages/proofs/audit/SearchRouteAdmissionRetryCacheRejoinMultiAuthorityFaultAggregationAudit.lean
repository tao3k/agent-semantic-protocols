import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinMultiAuthorityFaultAggregation

open ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinMultiAuthorityFaultAggregation

elab "writeSearchRouteAdmissionRetryCacheRejoinMultiAuthorityFaultAggregationAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-multi-authority-fault-aggregation-audit-v1.json"
    auditJson

writeSearchRouteAdmissionRetryCacheRejoinMultiAuthorityFaultAggregationAudit
