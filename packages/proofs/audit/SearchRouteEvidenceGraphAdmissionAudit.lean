import ASPProof.Audit.SearchRouteEvidenceGraphAdmission
import ASPProof.Audit.Receipt

open ASPProof.Audit

run_cmd do
  writeReceipt
    "receipts/searchroute-evidence-graph-admission-audit-v1.json"
    SearchRouteEvidenceGraphAdmission.auditJson
