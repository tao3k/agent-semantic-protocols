import ASPProof.Audit.SearchRouteSparseEvidenceGraph
import ASPProof.Audit.Receipt

open ASPProof.Audit

run_cmd do
  writeReceipt
    "receipts/searchroute-sparse-evidence-graph-audit-v1.json"
    SearchRouteSparseEvidenceGraph.auditJson
