import ASPProof.Audit.Receipt
import ASPProof.SearchRouteAdmissionRetryCacheRejoinQuorumCertificate

namespace ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinQuorumCertificate

open ASPProof.Audit.Core
open ASPProof.SearchRouteAdmissionRetryCacheRejoinQuorumCertificate

def targets : List Target :=
  [ Target.mk ``valid_same_slot_certificates_bind_same_receipt
      "same-slot-certificate-uniqueness"
      [ "ASP-RFC-10.05-CRQC-INTERSECTION", "ASP-RFC-10.05-CRQC-UNIQUENESS" ]
  , Target.mk ``nonquorum_certificate_is_not_valid
      "nonquorum-counterexample" [ "ASP-RFC-10.05-CRQC-QUORUM" ]
  , Target.mk ``inconsistent_signer_vote_invalidates_certificate
      "inconsistent-vote-counterexample" [ "ASP-RFC-10.05-CRQC-VOTE" ]
  , Target.mk ``valid_certificate_makes_receipt_recoverable
      "certified-recovery" [ "ASP-RFC-10.05-CRQC-RECOVERY" ]
  , Target.mk ``recoverable_same_slot_receipts_are_equal
      "recoverable-receipt-uniqueness"
      [ "ASP-RFC-10.05-CRQC-UNIQUENESS", "ASP-RFC-10.05-CRQC-RECOVERY" ]
  ]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryCacheRejoinQuorumCertificate"
    "ASPProof/SearchRouteAdmissionRetryCacheRejoinQuorumCertificate.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinQuorumCertificate
