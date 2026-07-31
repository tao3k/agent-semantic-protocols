import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinCanonicalMembership

open Lean Elab Command

elab "#writeSearchRouteAdmissionRetryCacheRejoinCanonicalMembershipAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-canonical-membership-audit-v1.json"
    ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinCanonicalMembership.auditJson

#writeSearchRouteAdmissionRetryCacheRejoinCanonicalMembershipAudit
