import ASPProof.Audit.SearchRouteGraphCostVector
import ASPProof.Audit.Receipt

open ASPProof.Audit

run_cmd do
  writeReceipt
    "receipts/searchroute-graph-cost-vector-audit-v1.json"
    SearchRouteGraphCostVector.auditJson
