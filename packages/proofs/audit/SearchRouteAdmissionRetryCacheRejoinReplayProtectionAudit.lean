import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinReplayProtection

open Lean Elab Command

elab "#writeSearchRouteAdmissionRetryCacheRejoinReplayProtectionAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-replay-protection-audit-v1.json"
    ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinReplayProtection.auditJson

#writeSearchRouteAdmissionRetryCacheRejoinReplayProtectionAudit
