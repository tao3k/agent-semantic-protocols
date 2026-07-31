import ASPProof.Audit.RuntimeSelectorOverlay

open Lean Elab Command

elab "#writeRuntimeSelectorOverlayAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/runtime-selector-overlay-audit-v1.json"
    ASPProof.Audit.RuntimeSelectorOverlay.auditJson

#writeRuntimeSelectorOverlayAudit
