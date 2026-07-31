import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinSinkAcceptance

open Lean Elab Command

elab "#writeSearchRouteAdmissionRetryCacheRejoinSinkAcceptanceAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-sink-acceptance-audit-v1.json"
    ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinSinkAcceptance.auditJson

#writeSearchRouteAdmissionRetryCacheRejoinSinkAcceptanceAudit
