-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteAdmissionRetryCacheRejoinSinkAcceptance

namespace ASPProof.SearchRouteAdmissionRetryCacheRejoinVerifierLifecycle

open ASPProof.SearchRouteAdmissionRetryCacheRejoinPublicationOutbox
open ASPProof.SearchRouteAdmissionRetryCacheRejoinSinkAcceptance

abbrev TrustEpoch := Nat
abbrev TrustRevision := Nat

structure TrustBoundIdentity (Digest : Type) where
  delivery : DeliveryIdentity Digest
  trustEpoch : TrustEpoch
  issuedAtRevision : TrustRevision

structure LifecycleVerifier (Digest : Type) where
  Evidence : Type
  verifies : TrustBoundIdentity Digest → Evidence → Prop

structure LifecycleAcceptanceReceipt {Digest : Type}
    (verifier : LifecycleVerifier Digest) where
  identity : TrustBoundIdentity Digest
  evidence : verifier.Evidence

structure TrustLifecycle where
  currentEpoch : TrustEpoch
  previousEpoch : Option TrustEpoch
  overlapEndsAtRevision : TrustRevision
  revokedAt : TrustEpoch → Option TrustRevision

def EpochInCurrentWindow
    (lifecycle : TrustLifecycle)
    (epoch : TrustEpoch)
    (verificationRevision : TrustRevision) : Prop :=
  epoch = lifecycle.currentEpoch ∨
  (lifecycle.previousEpoch = some epoch ∧
    verificationRevision ≤ lifecycle.overlapEndsAtRevision)

def NotRevokedAt
    (lifecycle : TrustLifecycle)
    (epoch : TrustEpoch)
    (revision : TrustRevision) : Prop :=
  match lifecycle.revokedAt epoch with
  | none => True
  | some effectiveRevision => revision < effectiveRevision

def CanAuthorizeCurrent
    (lifecycle : TrustLifecycle)
    (verificationRevision : TrustRevision)
    (identity : TrustBoundIdentity Digest) : Prop :=
  identity.issuedAtRevision ≤ verificationRevision ∧
  EpochInCurrentWindow
    lifecycle identity.trustEpoch verificationRevision ∧
  NotRevokedAt
    lifecycle identity.trustEpoch verificationRevision

def HistoricallyValid
    (verifier : LifecycleVerifier Digest)
    (lifecycle : TrustLifecycle)
    (receipt : LifecycleAcceptanceReceipt verifier) : Prop :=
  verifier.verifies receipt.identity receipt.evidence ∧
  NotRevokedAt
    lifecycle
    receipt.identity.trustEpoch
    receipt.identity.issuedAtRevision

def CanAcknowledgeWithLifecycle
    (verifier : LifecycleVerifier Digest)
    (lifecycle : TrustLifecycle)
    (verificationRevision : TrustRevision)
    (publication : SinkBoundPublication Digest)
    (receipt : LifecycleAcceptanceReceipt verifier) : Prop :=
  CanAcknowledge publication.bundle ∧
  receipt.identity.delivery.sinkId = publication.sinkId ∧
  receipt.identity.delivery.protocolVersion = publication.protocolVersion ∧
  receipt.identity.delivery.edge = publication.bundle.obligation.edge ∧
  verifier.verifies receipt.identity receipt.evidence ∧
  CanAuthorizeCurrent
    lifecycle verificationRevision receipt.identity

def acknowledgeWithLifecycle
    (verifier : LifecycleVerifier Digest)
    (lifecycle : TrustLifecycle)
    (verificationRevision : TrustRevision)
    (publication : SinkBoundPublication Digest)
    (receipt : LifecycleAcceptanceReceipt verifier)
    (admitted :
      CanAcknowledgeWithLifecycle
        verifier lifecycle verificationRevision publication receipt) :
    PublicationBundle Digest :=
  acknowledge publication.bundle admitted.1

theorem lifecycle_acknowledgement_requires_bound_verified_current_identity
    (verifier : LifecycleVerifier Digest)
    (lifecycle : TrustLifecycle)
    (verificationRevision : TrustRevision)
    (publication : SinkBoundPublication Digest)
    (receipt : LifecycleAcceptanceReceipt verifier)
    (admitted :
      CanAcknowledgeWithLifecycle
        verifier lifecycle verificationRevision publication receipt) :
    receipt.identity.delivery.sinkId = publication.sinkId ∧
    receipt.identity.delivery.protocolVersion = publication.protocolVersion ∧
    receipt.identity.delivery.edge = publication.bundle.obligation.edge ∧
    verifier.verifies receipt.identity receipt.evidence ∧
    CanAuthorizeCurrent
      lifecycle verificationRevision receipt.identity := by
  rcases admitted with
    ⟨_, sinkMatches, versionMatches, edgeMatches, verified, current⟩
  exact
    ⟨sinkMatches, versionMatches, edgeMatches, verified, current⟩

theorem lifecycle_verified_receipt_closes_obligation
    (verifier : LifecycleVerifier Digest)
    (lifecycle : TrustLifecycle)
    (verificationRevision : TrustRevision)
    (publication : SinkBoundPublication Digest)
    (receipt : LifecycleAcceptanceReceipt verifier)
    (admitted :
      CanAcknowledgeWithLifecycle
        verifier lifecycle verificationRevision publication receipt) :
    (acknowledgeWithLifecycle verifier lifecycle verificationRevision publication receipt admitted).obligation.phase =
      PublicationPhase.acknowledged :=
  rfl

theorem future_issued_identity_cannot_authorize_current
    (lifecycle : TrustLifecycle)
    (verificationRevision : TrustRevision)
    (identity : TrustBoundIdentity Digest)
    (futureIssued :
      verificationRevision < identity.issuedAtRevision) :
    ¬ CanAuthorizeCurrent lifecycle verificationRevision identity := by
  intro authorized
  exact (Nat.not_le_of_gt futureIssued) authorized.1

theorem unlisted_trust_epoch_cannot_authorize_current
    (lifecycle : TrustLifecycle)
    (verificationRevision : TrustRevision)
    (identity : TrustBoundIdentity Digest)
    (notCurrent : identity.trustEpoch ≠ lifecycle.currentEpoch)
    (notPrevious :
      lifecycle.previousEpoch ≠ some identity.trustEpoch) :
    ¬ CanAuthorizeCurrent lifecycle verificationRevision identity := by
  intro authorized
  rcases authorized.2.1 with current | previous
  · exact notCurrent current
  · exact notPrevious previous.1

theorem previous_epoch_after_overlap_cannot_authorize_current
    (lifecycle : TrustLifecycle)
    (verificationRevision : TrustRevision)
    (identity : TrustBoundIdentity Digest)
    (notCurrent : identity.trustEpoch ≠ lifecycle.currentEpoch)
    (afterOverlap :
      lifecycle.overlapEndsAtRevision < verificationRevision) :
    ¬ CanAuthorizeCurrent lifecycle verificationRevision identity := by
  intro authorized
  rcases authorized.2.1 with current | previous
  · exact notCurrent current
  · exact (Nat.not_le_of_gt afterOverlap) previous.2

theorem revoked_epoch_cannot_authorize_at_or_after_effective_revision
    (lifecycle : TrustLifecycle)
    (verificationRevision effectiveRevision : TrustRevision)
    (identity : TrustBoundIdentity Digest)
    (revoked :
      lifecycle.revokedAt identity.trustEpoch =
        some effectiveRevision)
    (effective :
      effectiveRevision ≤ verificationRevision) :
    ¬ CanAuthorizeCurrent lifecycle verificationRevision identity := by
  intro authorized
  have notRevoked := authorized.2.2
  unfold NotRevokedAt at notRevoked
  rw [revoked] at notRevoked
  exact (Nat.not_lt_of_ge effective) notRevoked

theorem pre_revocation_verified_receipt_remains_historically_valid
    (verifier : LifecycleVerifier Digest)
    (lifecycle : TrustLifecycle)
    (receipt : LifecycleAcceptanceReceipt verifier)
    (verified :
      verifier.verifies receipt.identity receipt.evidence)
    (effectiveRevision : TrustRevision)
    (revoked :
      lifecycle.revokedAt receipt.identity.trustEpoch =
        some effectiveRevision)
    (issuedBefore :
      receipt.identity.issuedAtRevision < effectiveRevision) :
    HistoricallyValid verifier lifecycle receipt := by
  constructor
  · exact verified
  · unfold NotRevokedAt
    rw [revoked]
    exact issuedBefore

theorem receipt_issued_at_or_after_revocation_is_not_historically_valid
    (verifier : LifecycleVerifier Digest)
    (lifecycle : TrustLifecycle)
    (receipt : LifecycleAcceptanceReceipt verifier)
    (effectiveRevision : TrustRevision)
    (revoked :
      lifecycle.revokedAt receipt.identity.trustEpoch =
        some effectiveRevision)
    (issuedAfter :
      effectiveRevision ≤ receipt.identity.issuedAtRevision) :
    ¬ HistoricallyValid verifier lifecycle receipt := by
  intro historical
  have notRevoked := historical.2
  unfold NotRevokedAt at notRevoked
  rw [revoked] at notRevoked
  exact (Nat.not_lt_of_ge issuedAfter) notRevoked

theorem historical_validity_does_not_imply_current_authorization_after_revocation
    (verifier : LifecycleVerifier Digest)
    (lifecycle : TrustLifecycle)
    (receipt : LifecycleAcceptanceReceipt verifier)
    (verificationRevision effectiveRevision : TrustRevision)
    (verified :
      verifier.verifies receipt.identity receipt.evidence)
    (revoked :
      lifecycle.revokedAt receipt.identity.trustEpoch =
        some effectiveRevision)
    (issuedBefore :
      receipt.identity.issuedAtRevision < effectiveRevision)
    (verificationAfter :
      effectiveRevision ≤ verificationRevision) :
    HistoricallyValid verifier lifecycle receipt ∧
    ¬ CanAuthorizeCurrent
      lifecycle verificationRevision receipt.identity := by
  constructor
  · exact
      pre_revocation_verified_receipt_remains_historically_valid
        verifier lifecycle receipt verified effectiveRevision
        revoked issuedBefore
  · exact
      revoked_epoch_cannot_authorize_at_or_after_effective_revision
        lifecycle verificationRevision effectiveRevision
        receipt.identity revoked verificationAfter

end ASPProof.SearchRouteAdmissionRetryCacheRejoinVerifierLifecycle
