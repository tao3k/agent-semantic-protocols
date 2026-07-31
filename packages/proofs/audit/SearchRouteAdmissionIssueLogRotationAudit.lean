import ASPProof.Audit.Receipt
import ASPProof.Audit.SearchRouteAdmissionIssueLogRotation

open Lean Elab Command

elab "writeSearchRouteAdmissionIssueLogRotationAudit" : command => do
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-issue-log-rotation-audit-v1.json"
    ASPProof.Audit.SearchRouteAdmissionIssueLogRotation.auditJson

writeSearchRouteAdmissionIssueLogRotationAudit
