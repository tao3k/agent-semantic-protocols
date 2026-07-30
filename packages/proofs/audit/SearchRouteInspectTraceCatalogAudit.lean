import ASPProof.Audit.SearchRouteInspectTraceCatalog
import ASPProof.Audit.Receipt

open ASPProof.Audit

run_cmd do
  writeReceipt
    "receipts/searchroute-inspect-trace-catalog-audit-v1.json"
    SearchRouteInspectTraceCatalog.auditJson
