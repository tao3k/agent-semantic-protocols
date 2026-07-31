import ASPProof.Audit.Receipt
import ASPProof.Audit.SearchRouteAdmissionRetryAuthorizationStamp

open Lean Elab Command

elab "writeSearchRouteAdmissionRetryAuthorizationStampAudit" : command => do
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-authorization-stamp-audit-v1.json"
    ASPProof.Audit.SearchRouteAdmissionRetryAuthorizationStamp.auditJson

writeSearchRouteAdmissionRetryAuthorizationStampAudit
