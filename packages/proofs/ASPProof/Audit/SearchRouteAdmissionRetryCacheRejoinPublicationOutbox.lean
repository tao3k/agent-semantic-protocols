import ASPProof.Audit.Receipt
import ASPProof.SearchRouteAdmissionRetryCacheRejoinPublicationOutbox

namespace ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinPublicationOutbox

open ASPProof.Audit.Core
open ASPProof.SearchRouteAdmissionRetryCacheRejoinPublicationOutbox

def targets : List Target :=
  [ Target.mk
      ``atomic_activation_commit_creates_authorized_pending_obligation
      "atomic-pending-obligation"
      [ "ASP-RFC-10.05-CRPO-ATOMIC"
      , "ASP-RFC-10.05-CRPO-AUTHORIZATION"
      ]
  , Target.mk
      ``pending_commit_is_recoverably_publishable
      "pending-crash-recovery"
      [ "ASP-RFC-10.05-CRPO-PENDING" ]
  , Target.mk
      ``publication_preserves_edge_and_marks_published
      "stable-edge-publication"
      [ "ASP-RFC-10.05-CRPO-RETRY" ]
  , Target.mk
      ``published_bundle_remains_retryable
      "published-retry-admission"
      [ "ASP-RFC-10.05-CRPO-RETRY" ]
  , Target.mk
      ``publication_retry_is_idempotent
      "idempotent-publication-retry"
      [ "ASP-RFC-10.05-CRPO-RETRY" ]
  , Target.mk
      ``stale_revision_obligation_cannot_publish
      "stale-obligation-counterexample"
      [ "ASP-RFC-10.05-CRPO-AUTHORIZATION" ]
  , Target.mk
      ``published_bundle_can_be_acknowledged
      "published-ack-admission"
      [ "ASP-RFC-10.05-CRPO-ACK" ]
  , Target.mk
      ``acknowledgement_preserves_edge_and_closes_obligation
      "acknowledgement-closure"
      [ "ASP-RFC-10.05-CRPO-ACK"
      , "ASP-RFC-10.05-CRPO-CLOSE"
      ]
  , Target.mk
      ``acknowledged_bundle_cannot_publish_again
      "post-ack-publication-counterexample"
      [ "ASP-RFC-10.05-CRPO-CLOSE" ]
  , Target.mk
      ``acknowledged_bundle_cannot_close_twice
      "duplicate-closure-counterexample"
      [ "ASP-RFC-10.05-CRPO-CLOSE" ]
  ]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryCacheRejoinPublicationOutbox"
    "ASPProof/SearchRouteAdmissionRetryCacheRejoinPublicationOutbox.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinPublicationOutbox
