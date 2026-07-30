import ASPProof.Audit.SearchRouteEvidenceGraphRouter
import ASPProof.Audit.Receipt

open ASPProof.Audit

run_cmd do
  writeReceipt
    "receipts/searchroute-evidence-graph-router-audit-v1.json"
    SearchRouteEvidenceGraphRouter.auditJson
