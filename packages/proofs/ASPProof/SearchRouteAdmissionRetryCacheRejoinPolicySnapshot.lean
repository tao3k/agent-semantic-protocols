import ASPProof.SearchRouteAdmissionRetryCacheRejoinVerifierLifecycle

namespace ASPProof.SearchRouteAdmissionRetryCacheRejoinPolicySnapshot

open ASPProof.SearchRouteAdmissionRetryCacheRejoinPublicationOutbox
open ASPProof.SearchRouteAdmissionRetryCacheRejoinSinkAcceptance
open ASPProof.SearchRouteAdmissionRetryCacheRejoinVerifierLifecycle

structure PolicyDigestScheme where
  Digest : Type
  commit : TrustLifecycle → Digest
  collisionFree : Function.Injective commit

structure BoundPolicySnapshot (scheme : PolicyDigestScheme) where
  lifecycle : TrustLifecycle
  revision : TrustRevision
  digest : scheme.Digest
  bound : digest = scheme.commit lifecycle

structure AcknowledgementProposal {Digest : Type}
    (verifier : LifecycleVerifier Digest)
    (scheme : PolicyDigestScheme) where
  expectedAdmissionRevision : Nat
  expectedPolicyRevision : TrustRevision
  expectedPolicyDigest : scheme.Digest
  receipt : LifecycleAcceptanceReceipt verifier

def CanCommitAcknowledgement {Digest : Type}
    (verifier : LifecycleVerifier Digest)
    (scheme : PolicyDigestScheme)
    (policy : BoundPolicySnapshot scheme)
    (publication : SinkBoundPublication Digest)
    (proposal : AcknowledgementProposal verifier scheme) : Prop :=
  proposal.expectedAdmissionRevision =
      publication.bundle.admission.revision ∧
  proposal.expectedPolicyRevision = policy.revision ∧
  proposal.expectedPolicyDigest = policy.digest ∧
  CanAcknowledgeWithLifecycle
    verifier
    policy.lifecycle
    policy.revision
    publication
    proposal.receipt

structure CommittedAcknowledgement {Digest : Type}
    (verifier : LifecycleVerifier Digest)
    (scheme : PolicyDigestScheme) where
  closedBundle : PublicationBundle Digest
  admissionRevision : Nat
  policyRevision : TrustRevision
  policyDigest : scheme.Digest
  receipt : LifecycleAcceptanceReceipt verifier

def commitAcknowledgement {Digest : Type}
    (verifier : LifecycleVerifier Digest)
    (scheme : PolicyDigestScheme)
    (policy : BoundPolicySnapshot scheme)
    (publication : SinkBoundPublication Digest)
    (proposal : AcknowledgementProposal verifier scheme)
    (committable :
      CanCommitAcknowledgement
        verifier scheme policy publication proposal) :
    CommittedAcknowledgement verifier scheme where
  closedBundle :=
    acknowledgeWithLifecycle
      verifier
      policy.lifecycle
      policy.revision
      publication
      proposal.receipt
      committable.2.2.2
  admissionRevision := publication.bundle.admission.revision
  policyRevision := policy.revision
  policyDigest := policy.digest
  receipt := proposal.receipt

theorem committable_acknowledgement_is_bound_to_current_snapshots
    {Digest : Type}
    (verifier : LifecycleVerifier Digest)
    (scheme : PolicyDigestScheme)
    (policy : BoundPolicySnapshot scheme)
    (publication : SinkBoundPublication Digest)
    (proposal : AcknowledgementProposal verifier scheme)
    (committable :
      CanCommitAcknowledgement
        verifier scheme policy publication proposal) :
    proposal.expectedAdmissionRevision =
        publication.bundle.admission.revision ∧
    proposal.expectedPolicyRevision = policy.revision ∧
    proposal.expectedPolicyDigest = policy.digest ∧
    CanAcknowledgeWithLifecycle
      verifier policy.lifecycle policy.revision publication proposal.receipt :=
  committable

theorem committed_acknowledgement_records_snapshots_and_closes
    {Digest : Type}
    (verifier : LifecycleVerifier Digest)
    (scheme : PolicyDigestScheme)
    (policy : BoundPolicySnapshot scheme)
    (publication : SinkBoundPublication Digest)
    (proposal : AcknowledgementProposal verifier scheme)
    (committable :
      CanCommitAcknowledgement
        verifier scheme policy publication proposal) :
    (commitAcknowledgement verifier scheme policy publication proposal committable).admissionRevision =
      publication.bundle.admission.revision ∧
    (commitAcknowledgement verifier scheme policy publication proposal committable).policyRevision =
      policy.revision ∧
    (commitAcknowledgement verifier scheme policy publication proposal committable).policyDigest =
      policy.digest ∧
    (commitAcknowledgement verifier scheme policy publication proposal committable).closedBundle.obligation.phase =
      PublicationPhase.acknowledged :=
  ⟨rfl, rfl, rfl, rfl⟩

theorem stale_admission_snapshot_cannot_commit_acknowledgement
    {Digest : Type}
    (verifier : LifecycleVerifier Digest)
    (scheme : PolicyDigestScheme)
    (policy : BoundPolicySnapshot scheme)
    (publication : SinkBoundPublication Digest)
    (proposal : AcknowledgementProposal verifier scheme)
    (stale :
      proposal.expectedAdmissionRevision <
        publication.bundle.admission.revision) :
    ¬ CanCommitAcknowledgement
      verifier scheme policy publication proposal := by
  intro committable
  exact (Nat.ne_of_lt stale) committable.1

theorem stale_policy_revision_cannot_commit_acknowledgement
    {Digest : Type}
    (verifier : LifecycleVerifier Digest)
    (scheme : PolicyDigestScheme)
    (policy : BoundPolicySnapshot scheme)
    (publication : SinkBoundPublication Digest)
    (proposal : AcknowledgementProposal verifier scheme)
    (stale :
      proposal.expectedPolicyRevision < policy.revision) :
    ¬ CanCommitAcknowledgement
      verifier scheme policy publication proposal := by
  intro committable
  exact (Nat.ne_of_lt stale) committable.2.1

theorem wrong_policy_digest_cannot_commit_acknowledgement
    {Digest : Type}
    (verifier : LifecycleVerifier Digest)
    (scheme : PolicyDigestScheme)
    (policy : BoundPolicySnapshot scheme)
    (publication : SinkBoundPublication Digest)
    (proposal : AcknowledgementProposal verifier scheme)
    (wrongDigest :
      proposal.expectedPolicyDigest ≠ policy.digest) :
    ¬ CanCommitAcknowledgement
      verifier scheme policy publication proposal := by
  intro committable
  exact wrongDigest committable.2.2.1

theorem newer_policy_snapshot_rejects_old_validation
    {Digest : Type}
    (verifier : LifecycleVerifier Digest)
    (scheme : PolicyDigestScheme)
    (oldPolicy newPolicy : BoundPolicySnapshot scheme)
    (publication : SinkBoundPublication Digest)
    (proposal : AcknowledgementProposal verifier scheme)
    (observedOld :
      proposal.expectedPolicyRevision = oldPolicy.revision)
    (advanced : oldPolicy.revision < newPolicy.revision) :
    ¬ CanCommitAcknowledgement
      verifier scheme newPolicy publication proposal := by
  intro committable
  have oldEqualsNew :
      oldPolicy.revision = newPolicy.revision :=
    observedOld.symm.trans committable.2.1
  exact (Nat.ne_of_lt advanced) oldEqualsNew

theorem changed_admission_snapshot_rejects_old_validation
    {Digest : Type}
    (verifier : LifecycleVerifier Digest)
    (scheme : PolicyDigestScheme)
    (policy : BoundPolicySnapshot scheme)
    (publication : SinkBoundPublication Digest)
    (proposal : AcknowledgementProposal verifier scheme)
    (observedRevision : Nat)
    (observed :
      proposal.expectedAdmissionRevision = observedRevision)
    (changed :
      observedRevision ≠ publication.bundle.admission.revision) :
    ¬ CanCommitAcknowledgement
      verifier scheme policy publication proposal := by
  intro committable
  exact changed (observed.symm.trans committable.1)

theorem equal_bound_policy_digest_implies_equal_lifecycle
    (scheme : PolicyDigestScheme)
    (left right : BoundPolicySnapshot scheme)
    (sameDigest : left.digest = right.digest) :
    left.lifecycle = right.lifecycle := by
  apply scheme.collisionFree
  calc
    scheme.commit left.lifecycle = left.digest := left.bound.symm
    _ = right.digest := sameDigest
    _ = scheme.commit right.lifecycle := right.bound

end ASPProof.SearchRouteAdmissionRetryCacheRejoinPolicySnapshot
