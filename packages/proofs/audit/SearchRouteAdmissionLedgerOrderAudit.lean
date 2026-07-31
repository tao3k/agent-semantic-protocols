import ASPProof.Audit.Receipt
import ASPProof.Audit.SearchRouteAdmissionLedgerOrder

open Lean Elab Command

elab "writeSearchRouteAdmissionLedgerOrderAudit" : command => do
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-ledger-order-audit-v1.json"
    ASPProof.Audit.SearchRouteAdmissionLedgerOrder.auditJson

writeSearchRouteAdmissionLedgerOrderAudit
