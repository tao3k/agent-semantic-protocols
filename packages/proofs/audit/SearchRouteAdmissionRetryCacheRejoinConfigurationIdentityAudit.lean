import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinConfigurationIdentity

open Lean Elab Command

elab "#writeSearchRouteAdmissionRetryCacheRejoinConfigurationIdentityAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-configuration-identity-audit-v1.json"
    ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinConfigurationIdentity.auditJson

#writeSearchRouteAdmissionRetryCacheRejoinConfigurationIdentityAudit
