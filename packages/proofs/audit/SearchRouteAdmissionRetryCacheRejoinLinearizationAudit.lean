import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinLinearization

open Lean Elab Command

elab "#writeSearchRouteAdmissionRetryCacheRejoinLinearizationAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-linearization-audit-v1.json"
    ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinLinearization.auditJson

#writeSearchRouteAdmissionRetryCacheRejoinLinearizationAudit
