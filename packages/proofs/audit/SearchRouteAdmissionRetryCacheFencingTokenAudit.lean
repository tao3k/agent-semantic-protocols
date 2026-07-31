import ASPProof.Audit.Receipt
import ASPProof.Audit.SearchRouteAdmissionRetryCacheFencingToken

open Lean Elab Command

elab "writeSearchRouteAdmissionRetryCacheFencingTokenAudit" : command => do
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-fencing-token-audit-v1.json"
    ASPProof.Audit.SearchRouteAdmissionRetryCacheFencingToken.auditJson

writeSearchRouteAdmissionRetryCacheFencingTokenAudit
