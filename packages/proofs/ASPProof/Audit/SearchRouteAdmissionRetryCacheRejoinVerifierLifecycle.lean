import ASPProof.Audit.Receipt
import ASPProof.SearchRouteAdmissionRetryCacheRejoinVerifierLifecycle

namespace ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinVerifierLifecycle

open ASPProof.Audit.Core
open ASPProof.SearchRouteAdmissionRetryCacheRejoinVerifierLifecycle

def targets : List Target :=
  [ Target.mk
      ``lifecycle_acknowledgement_requires_bound_verified_current_identity
      "lifecycle-acknowledgement-gate"
      [ "ASP-RFC-10.05-CRVL-BINDING"
      , "ASP-RFC-10.05-CRVL-ACK"
      ]
  , Target.mk
      ``lifecycle_verified_receipt_closes_obligation
      "lifecycle-verified-closure"
      [ "ASP-RFC-10.05-CRVL-ACK" ]
  , Target.mk
      ``future_issued_identity_cannot_authorize_current
      "future-issuance-counterexample"
      [ "ASP-RFC-10.05-CRVL-TIME" ]
  , Target.mk
      ``unlisted_trust_epoch_cannot_authorize_current
      "unlisted-epoch-counterexample"
      [ "ASP-RFC-10.05-CRVL-CURRENT" ]
  , Target.mk
      ``previous_epoch_after_overlap_cannot_authorize_current
      "expired-overlap-counterexample"
      [ "ASP-RFC-10.05-CRVL-OVERLAP" ]
  , Target.mk
      ``revoked_epoch_cannot_authorize_at_or_after_effective_revision
      "current-revocation-counterexample"
      [ "ASP-RFC-10.05-CRVL-REVOCATION" ]
  , Target.mk
      ``pre_revocation_verified_receipt_remains_historically_valid
      "historical-pre-revocation-validity"
      [ "ASP-RFC-10.05-CRVL-HISTORY" ]
  , Target.mk
      ``receipt_issued_at_or_after_revocation_is_not_historically_valid
      "post-revocation-history-counterexample"
      [ "ASP-RFC-10.05-CRVL-REVOCATION"
      , "ASP-RFC-10.05-CRVL-HISTORY"
      ]
  , Target.mk
      ``historical_validity_does_not_imply_current_authorization_after_revocation
      "historical-current-separation"
      [ "ASP-RFC-10.05-CRVL-HISTORY" ]
  ]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryCacheRejoinVerifierLifecycle"
    "ASPProof/SearchRouteAdmissionRetryCacheRejoinVerifierLifecycle.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinVerifierLifecycle
