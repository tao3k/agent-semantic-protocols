import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinJointWeightScheduleTransition

open ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinJointWeightScheduleTransition

elab "writeSearchRouteAdmissionRetryCacheRejoinJointWeightScheduleTransitionAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-joint-weight-schedule-transition-audit-v1.json"
    auditJson

writeSearchRouteAdmissionRetryCacheRejoinJointWeightScheduleTransitionAudit
