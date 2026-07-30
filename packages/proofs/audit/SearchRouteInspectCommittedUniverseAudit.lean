import ASPProof.Audit.SearchRouteInspectCommittedUniverse
import ASPProof.Audit.Receipt

open ASPProof.Audit

run_cmd do
  writeReceipt
    "receipts/searchroute-inspect-committed-universe-audit-v1.json"
    SearchRouteInspectCommittedUniverse.auditJson
