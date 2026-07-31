import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinVerifierLifecycle

open Lean Elab Command

elab "#writeSearchRouteAdmissionRetryCacheRejoinVerifierLifecycleAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-verifier-lifecycle-audit-v1.json"
    ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinVerifierLifecycle.auditJson

#writeSearchRouteAdmissionRetryCacheRejoinVerifierLifecycleAudit
