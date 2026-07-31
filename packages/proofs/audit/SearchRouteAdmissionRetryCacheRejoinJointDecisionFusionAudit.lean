import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinJointDecisionFusion

open ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinJointDecisionFusion

elab "writeSearchRouteAdmissionRetryCacheRejoinJointDecisionFusionAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-joint-decision-fusion-audit-v1.json"
    auditJson

writeSearchRouteAdmissionRetryCacheRejoinJointDecisionFusionAudit
