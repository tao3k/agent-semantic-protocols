import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinQuorumRealization

open ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinQuorumRealization

elab "writeSearchRouteAdmissionRetryCacheRejoinQuorumRealizationAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-quorum-realization-audit-v1.json"
    auditJson

writeSearchRouteAdmissionRetryCacheRejoinQuorumRealizationAudit
