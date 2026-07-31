import ASPProof.Audit.Receipt
import ASPProof.Audit.SearchRouteAdmissionRetryPublicationLedger

open Lean Elab Command

elab "writeSearchRouteAdmissionRetryPublicationLedgerAudit" : command => do
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-publication-ledger-audit-v1.json"
    ASPProof.Audit.SearchRouteAdmissionRetryPublicationLedger.auditJson

writeSearchRouteAdmissionRetryPublicationLedgerAudit
