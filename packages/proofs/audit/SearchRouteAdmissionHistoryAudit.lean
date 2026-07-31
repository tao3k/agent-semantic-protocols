import ASPProof.Audit.Receipt
import ASPProof.Audit.SearchRouteAdmissionHistory

open Lean Elab Command

run_cmd
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-history-audit-v1.json"
    (ASPProof.Audit.Core.proofAuditJson
      "ASPProof.SearchRouteAdmissionHistory"
      "packages/proofs/ASPProof/SearchRouteAdmissionHistory.lean"
      ASPProof.Audit.SearchRouteAdmissionHistory.targets)
