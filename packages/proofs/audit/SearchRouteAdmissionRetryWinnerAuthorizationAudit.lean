import ASPProof.Audit.Receipt
import ASPProof.Audit.SearchRouteAdmissionRetryWinnerAuthorization

open Lean Elab Command

elab "writeSearchRouteAdmissionRetryWinnerAuthorizationAudit" : command => do
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-winner-authorization-audit-v1.json"
    ASPProof.Audit.SearchRouteAdmissionRetryWinnerAuthorization.auditJson

writeSearchRouteAdmissionRetryWinnerAuthorizationAudit
