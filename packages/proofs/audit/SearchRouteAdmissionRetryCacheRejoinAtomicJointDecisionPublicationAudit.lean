import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinAtomicJointDecisionPublication

open ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinAtomicJointDecisionPublication

elab "writeSearchRouteAdmissionRetryCacheRejoinAtomicJointDecisionPublicationAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-atomic-joint-decision-publication-audit-v1.json"
    auditJson

writeSearchRouteAdmissionRetryCacheRejoinAtomicJointDecisionPublicationAudit
