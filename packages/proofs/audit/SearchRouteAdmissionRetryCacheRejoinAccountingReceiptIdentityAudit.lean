import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinAccountingReceiptIdentity

open ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinAccountingReceiptIdentity

elab "writeSearchRouteAdmissionRetryCacheRejoinAccountingReceiptIdentityAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-accounting-receipt-identity-audit-v1.json"
    auditJson

writeSearchRouteAdmissionRetryCacheRejoinAccountingReceiptIdentityAudit
