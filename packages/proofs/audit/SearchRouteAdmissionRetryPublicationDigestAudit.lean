import ASPProof.Audit.Receipt
import ASPProof.Audit.SearchRouteAdmissionRetryPublicationDigest

open Lean Elab Command

elab "writeSearchRouteAdmissionRetryPublicationDigestAudit" : command => do
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-publication-digest-audit-v1.json"
    ASPProof.Audit.SearchRouteAdmissionRetryPublicationDigest.auditJson

writeSearchRouteAdmissionRetryPublicationDigestAudit
