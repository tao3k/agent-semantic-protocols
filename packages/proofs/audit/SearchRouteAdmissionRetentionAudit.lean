import ASPProof.Audit.Receipt
import ASPProof.Audit.SearchRouteAdmissionRetention

open Lean Elab Command

run_cmd
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retention-audit-v1.json"
    (ASPProof.Audit.Core.proofAuditJson
      "ASPProof.SearchRouteAdmissionRetention"
      "packages/proofs/ASPProof/SearchRouteAdmissionRetention.lean"
      ASPProof.Audit.SearchRouteAdmissionRetention.targets)
