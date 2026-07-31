import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinPublicationOutbox

open Lean Elab Command

elab "#writeSearchRouteAdmissionRetryCacheRejoinPublicationOutboxAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-publication-outbox-audit-v1.json"
    ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinPublicationOutbox.auditJson

#writeSearchRouteAdmissionRetryCacheRejoinPublicationOutboxAudit
