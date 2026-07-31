import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinFenceAuthority

open Lean Elab Command

elab "#writeSearchRouteAdmissionRetryCacheRejoinFenceAuthorityAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-fence-authority-audit-v1.json"
    ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinFenceAuthority.auditJson

#writeSearchRouteAdmissionRetryCacheRejoinFenceAuthorityAudit
