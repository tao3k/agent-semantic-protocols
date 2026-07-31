import ASPProof.Audit.Receipt
import ASPProof.Audit.SearchRouteAdmissionRetryPublicationKey

open Lean Elab Command

elab "writeSearchRouteAdmissionRetryPublicationKeyAudit" : command => do
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-publication-key-audit-v1.json"
    ASPProof.Audit.SearchRouteAdmissionRetryPublicationKey.auditJson

writeSearchRouteAdmissionRetryPublicationKeyAudit
