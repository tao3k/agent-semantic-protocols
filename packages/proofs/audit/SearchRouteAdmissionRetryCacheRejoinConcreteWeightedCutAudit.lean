import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinConcreteWeightedCut

open ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinConcreteWeightedCut

elab "writeSearchRouteAdmissionRetryCacheRejoinConcreteWeightedCutAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-concrete-weighted-cut-audit-v1.json"
    auditJson

writeSearchRouteAdmissionRetryCacheRejoinConcreteWeightedCutAudit
