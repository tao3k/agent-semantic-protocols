import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinQuorumIntersection

open ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinQuorumIntersection

elab "writeSearchRouteAdmissionRetryCacheRejoinQuorumIntersectionAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-quorum-intersection-audit-v1.json"
    auditJson

writeSearchRouteAdmissionRetryCacheRejoinQuorumIntersectionAudit
