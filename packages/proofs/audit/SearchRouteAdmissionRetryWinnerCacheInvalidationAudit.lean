import ASPProof.Audit.Receipt
import ASPProof.Audit.SearchRouteAdmissionRetryWinnerCacheInvalidation

open Lean Elab Command

elab "writeSearchRouteAdmissionRetryWinnerCacheInvalidationAudit" : command => do
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-winner-cache-invalidation-audit-v1.json"
    ASPProof.Audit.SearchRouteAdmissionRetryWinnerCacheInvalidation.auditJson

writeSearchRouteAdmissionRetryWinnerCacheInvalidationAudit
