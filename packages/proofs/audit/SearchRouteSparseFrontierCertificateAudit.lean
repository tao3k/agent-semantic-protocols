import ASPProof.Audit.SearchRouteSparseFrontierCertificate
import ASPProof.Audit.Receipt

open ASPProof.Audit

run_cmd do
  writeReceipt
    "receipts/searchroute-sparse-frontier-certificate-audit-v1.json"
    SearchRouteSparseFrontierCertificate.auditJson
