import ASPProof.Audit.ActivationAdmission

open Lean Elab Command

elab "#writeActivationAdmissionAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/activation-admission-audit-v1.json"
    ASPProof.Audit.ActivationAdmission.auditJson

#writeActivationAdmissionAudit
