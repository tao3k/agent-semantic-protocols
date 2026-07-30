import ASPProof.Audit.SearchRouteInspectFiniteCommitment
import ASPProof.Audit.Receipt

open ASPProof.Audit

run_cmd do
  writeReceipt
    "receipts/searchroute-inspect-finite-commitment-audit-v1.json"
    SearchRouteInspectFiniteCommitment.auditJson
