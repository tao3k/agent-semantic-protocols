import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinQuorumCertificate

open Lean Elab Command

elab "#writeSearchRouteAdmissionRetryCacheRejoinQuorumCertificateAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-quorum-certificate-audit-v1.json"
    ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinQuorumCertificate.auditJson

#writeSearchRouteAdmissionRetryCacheRejoinQuorumCertificateAudit
