import ASPProof.SearchRouteAdmissionRetryCacheRejoinPublicationOutbox

namespace ASPProof.SearchRouteAdmissionRetryCacheRejoinSinkAcceptance

open ASPProof.SearchRouteAdmissionRetryCacheRejoinLinearization
open ASPProof.SearchRouteAdmissionRetryCacheRejoinPublicationOutbox

abbrev SinkId := Nat
abbrev ProtocolVersion := Nat

structure DeliveryIdentity (Digest : Type) where
  sinkId : SinkId
  protocolVersion : ProtocolVersion
  edge : PublishedAdmissionEdge Digest

def deliveryIdentity {Digest : Type}
    (sinkId : SinkId)
    (protocolVersion : ProtocolVersion)
    (edge : PublishedAdmissionEdge Digest) :
    DeliveryIdentity Digest where
  sinkId := sinkId
  protocolVersion := protocolVersion
  edge := edge

theorem delivery_identity_eq_of_fields
    {Digest : Type}
    (left right : DeliveryIdentity Digest)
    (sameSink : left.sinkId = right.sinkId)
    (sameVersion : left.protocolVersion = right.protocolVersion)
    (sameEdge : left.edge = right.edge) :
    left = right := by
  cases left
  cases right
  simp_all

structure SinkVerifier (Digest : Type) where
  Evidence : Type
  verifies :
    SinkId →
    ProtocolVersion →
    PublishedAdmissionEdge Digest →
    Evidence →
    Prop

structure SinkAcceptanceReceipt {Digest : Type}
    (verifier : SinkVerifier Digest) where
  identity : DeliveryIdentity Digest
  evidence : verifier.Evidence

structure SinkBoundPublication (Digest : Type) where
  bundle : PublicationBundle Digest
  sinkId : SinkId
  protocolVersion : ProtocolVersion

def ReceiptAdmissionForExpectedTarget {Digest : Type}
    (verifier : SinkVerifier Digest)
    (expectedSink : SinkId)
    (expectedVersion : ProtocolVersion)
    (bundle : PublicationBundle Digest)
    (receipt : SinkAcceptanceReceipt verifier) : Prop :=
  CanAcknowledge bundle ∧
  receipt.identity.sinkId = expectedSink ∧
  receipt.identity.protocolVersion = expectedVersion ∧
  receipt.identity.edge = bundle.obligation.edge ∧
  verifier.verifies
    receipt.identity.sinkId
    receipt.identity.protocolVersion
    receipt.identity.edge
    receipt.evidence

def CanAcknowledgeWithReceipt {Digest : Type}
    (verifier : SinkVerifier Digest)
    (publication : SinkBoundPublication Digest)
    (receipt : SinkAcceptanceReceipt verifier) : Prop :=
  ReceiptAdmissionForExpectedTarget
    verifier
    publication.sinkId
    publication.protocolVersion
    publication.bundle
    receipt

structure VerifiedAcknowledgement {Digest : Type}
    (verifier : SinkVerifier Digest) where
  closedBundle : PublicationBundle Digest
  sinkId : SinkId
  protocolVersion : ProtocolVersion
  receipt : SinkAcceptanceReceipt verifier

def acknowledgeWithReceipt {Digest : Type}
    (verifier : SinkVerifier Digest)
    (publication : SinkBoundPublication Digest)
    (receipt : SinkAcceptanceReceipt verifier)
    (admitted :
      CanAcknowledgeWithReceipt verifier publication receipt) :
    VerifiedAcknowledgement verifier where
  closedBundle := acknowledge publication.bundle admitted.1
  sinkId := publication.sinkId
  protocolVersion := publication.protocolVersion
  receipt := receipt

theorem publication_retry_preserves_delivery_identity
    {Digest : Type}
    (sinkId : SinkId)
    (protocolVersion : ProtocolVersion)
    (bundle : PublicationBundle Digest)
    (publishable : CanPublish bundle) :
    deliveryIdentity sinkId protocolVersion
        (publish bundle publishable).obligation.edge =
      deliveryIdentity sinkId protocolVersion bundle.obligation.edge :=
  rfl

theorem admitted_receipt_matches_expected_delivery
    {Digest : Type}
    (verifier : SinkVerifier Digest)
    (publication : SinkBoundPublication Digest)
    (receipt : SinkAcceptanceReceipt verifier)
    (admitted :
      CanAcknowledgeWithReceipt verifier publication receipt) :
    receipt.identity =
      deliveryIdentity
        publication.sinkId
        publication.protocolVersion
        publication.bundle.obligation.edge := by
  apply delivery_identity_eq_of_fields
  · exact admitted.2.1
  · exact admitted.2.2.1
  · exact admitted.2.2.2.1

theorem verified_receipt_closes_publication_obligation
    {Digest : Type}
    (verifier : SinkVerifier Digest)
    (publication : SinkBoundPublication Digest)
    (receipt : SinkAcceptanceReceipt verifier)
    (admitted :
      CanAcknowledgeWithReceipt verifier publication receipt) :
    (acknowledgeWithReceipt verifier publication receipt admitted).closedBundle.obligation.phase =
        PublicationPhase.acknowledged :=
  rfl

theorem cross_sink_receipt_cannot_acknowledge
    {Digest : Type}
    (verifier : SinkVerifier Digest)
    (publication : SinkBoundPublication Digest)
    (receipt : SinkAcceptanceReceipt verifier)
    (crossSink : receipt.identity.sinkId ≠ publication.sinkId) :
    ¬ CanAcknowledgeWithReceipt verifier publication receipt := by
  intro admitted
  exact crossSink admitted.2.1

theorem wrong_protocol_version_receipt_cannot_acknowledge
    {Digest : Type}
    (verifier : SinkVerifier Digest)
    (publication : SinkBoundPublication Digest)
    (receipt : SinkAcceptanceReceipt verifier)
    (wrongVersion :
      receipt.identity.protocolVersion ≠ publication.protocolVersion) :
    ¬ CanAcknowledgeWithReceipt verifier publication receipt := by
  intro admitted
  exact wrongVersion admitted.2.2.1

theorem wrong_edge_receipt_cannot_acknowledge
    {Digest : Type}
    (verifier : SinkVerifier Digest)
    (publication : SinkBoundPublication Digest)
    (receipt : SinkAcceptanceReceipt verifier)
    (wrongEdge :
      receipt.identity.edge ≠ publication.bundle.obligation.edge) :
    ¬ CanAcknowledgeWithReceipt verifier publication receipt := by
  intro admitted
  exact wrongEdge admitted.2.2.2.1

theorem unverifiable_receipt_cannot_acknowledge
    {Digest : Type}
    (verifier : SinkVerifier Digest)
    (publication : SinkBoundPublication Digest)
    (receipt : SinkAcceptanceReceipt verifier)
    (unverifiable :
      ¬ verifier.verifies
        receipt.identity.sinkId
        receipt.identity.protocolVersion
        receipt.identity.edge
        receipt.evidence) :
    ¬ CanAcknowledgeWithReceipt verifier publication receipt := by
  intro admitted
  exact unverifiable admitted.2.2.2.2

theorem stale_edge_receipt_cannot_acknowledge
    {Digest : Type}
    (verifier : SinkVerifier Digest)
    (publication : SinkBoundPublication Digest)
    (receipt : SinkAcceptanceReceipt verifier)
    (stale :
      publication.bundle.obligation.edge.committedRevision <
        publication.bundle.admission.revision) :
    ¬ CanAcknowledgeWithReceipt verifier publication receipt := by
  intro admitted
  exact
    (stale_revision_edge_is_not_authorized
      publication.bundle.admission
      publication.bundle.obligation.edge
      stale)
      admitted.1.1

theorem receipt_closed_bundle_cannot_acknowledge_again
    {Digest : Type}
    (verifier : SinkVerifier Digest)
    (publication : SinkBoundPublication Digest)
    (receipt : SinkAcceptanceReceipt verifier)
    (admitted :
      CanAcknowledgeWithReceipt verifier publication receipt)
    (secondReceipt : SinkAcceptanceReceipt verifier) :
    ¬ CanAcknowledgeWithReceipt
      verifier
      { bundle :=
          (acknowledgeWithReceipt
            verifier publication receipt admitted).closedBundle
      , sinkId := publication.sinkId
      , protocolVersion := publication.protocolVersion
      }
      secondReceipt := by
  intro secondAdmission
  exact
    (acknowledged_bundle_cannot_close_twice publication.bundle admitted.1)
      secondAdmission.1

end ASPProof.SearchRouteAdmissionRetryCacheRejoinSinkAcceptance
