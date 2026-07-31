import ASPProof.Audit.ActivationLifecycleAudit

open Lean Elab Command

elab "#writeActivationLifecycleAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/activation-lifecycle-audit-v1.json"
    ASPProof.Audit.ActivationLifecycleAudit.auditJson

#writeActivationLifecycleAudit
