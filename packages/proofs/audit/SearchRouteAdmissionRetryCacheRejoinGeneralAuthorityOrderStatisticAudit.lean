import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinGeneralAuthorityOrderStatistic

open ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinGeneralAuthorityOrderStatistic

elab "writeSearchRouteAdmissionRetryCacheRejoinGeneralAuthorityOrderStatisticAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-general-authority-order-statistic-audit-v1.json"
    auditJson

writeSearchRouteAdmissionRetryCacheRejoinGeneralAuthorityOrderStatisticAudit
