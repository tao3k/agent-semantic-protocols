import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinReceipts

open Lean Elab Command

elab "#writeSearchRouteAdmissionRetryCacheRejoinReceiptsAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-receipts-audit-v1.json"
    ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinReceipts.auditJson

#writeSearchRouteAdmissionRetryCacheRejoinReceiptsAudit
