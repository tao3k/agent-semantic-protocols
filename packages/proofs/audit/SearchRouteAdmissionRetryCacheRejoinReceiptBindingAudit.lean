import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinReceiptBinding

open Lean Elab Command

elab "#writeSearchRouteAdmissionRetryCacheRejoinReceiptBindingAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-receipt-binding-audit-v1.json"
    ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinReceiptBinding.auditJson

#writeSearchRouteAdmissionRetryCacheRejoinReceiptBindingAudit
