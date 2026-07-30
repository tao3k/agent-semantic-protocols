import ASPProof.Audit.SearchRouteInspectCertifiedLedger
import ASPProof.Audit.Receipt

open ASPProof.Audit

run_cmd do
  writeReceipt
    "receipts/searchroute-inspect-certified-ledger-audit-v1.json"
    SearchRouteInspectCertifiedLedger.auditJson
