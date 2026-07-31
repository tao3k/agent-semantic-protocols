import ASPProof.Audit.Receipt
import ASPProof.Audit.SearchRouteAdmissionGcBarrier

open Lean Elab Command

run_cmd
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-gc-barrier-audit-v1.json"
    (ASPProof.Audit.Core.proofAuditJson
      "ASPProof.SearchRouteAdmissionGcBarrier"
      "packages/proofs/ASPProof/SearchRouteAdmissionGcBarrier.lean"
      ASPProof.Audit.SearchRouteAdmissionGcBarrier.targets)
