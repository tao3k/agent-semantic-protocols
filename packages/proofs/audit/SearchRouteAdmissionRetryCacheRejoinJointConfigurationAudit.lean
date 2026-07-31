import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinJointConfiguration

open Lean Elab Command

elab "#writeSearchRouteAdmissionRetryCacheRejoinJointConfigurationAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-joint-configuration-audit-v1.json"
    ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinJointConfiguration.auditJson

#writeSearchRouteAdmissionRetryCacheRejoinJointConfigurationAudit
