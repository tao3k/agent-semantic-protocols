import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinConcreteIntersection

open ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinConcreteIntersection

elab "writeSearchRouteAdmissionRetryCacheRejoinConcreteIntersectionAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-concrete-intersection-audit-v1.json"
    auditJson

writeSearchRouteAdmissionRetryCacheRejoinConcreteIntersectionAudit
