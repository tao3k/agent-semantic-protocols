import ASPProof.Audit.Receipt
import ASPProof.Audit.SearchRouteAdmissionRetryWinnerNoninterference

open Lean Elab Command

elab "writeSearchRouteAdmissionRetryWinnerNoninterferenceAudit" : command => do
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-winner-noninterference-audit-v1.json"
    ASPProof.Audit.SearchRouteAdmissionRetryWinnerNoninterference.auditJson

writeSearchRouteAdmissionRetryWinnerNoninterferenceAudit
