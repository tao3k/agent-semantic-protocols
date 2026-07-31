import ASPProof.Audit.Receipt
import ASPProof.Audit.SearchRouteAdmissionRetryPublication

open Lean Elab Command

elab "writeSearchRouteAdmissionRetryPublicationAudit" : command => do
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-publication-audit-v1.json"
    ASPProof.Audit.SearchRouteAdmissionRetryPublication.auditJson

writeSearchRouteAdmissionRetryPublicationAudit
