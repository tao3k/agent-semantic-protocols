import ASPProof.SearchRouteAdmissionRetryCacheRejoinPolicySnapshot

namespace ASPProof.SearchRouteAdmissionRetryCacheRejoinJointFence

open ASPProof.SearchRouteAdmissionRetryCacheRejoinPolicySnapshot
open ASPProof.SearchRouteAdmissionRetryCacheRejoinPublicationOutbox
open ASPProof.SearchRouteAdmissionRetryCacheRejoinSinkAcceptance
open ASPProof.SearchRouteAdmissionRetryCacheRejoinVerifierLifecycle

structure CompositeFence (PolicyDigest : Type) where
  sequence : Nat
  admissionRevision : Nat
  policyRevision : TrustRevision
  policyDigest : PolicyDigest

structure JointState (Digest : Type)
    (scheme : PolicyDigestScheme) where
  publication : SinkBoundPublication Digest
  policy : BoundPolicySnapshot scheme
  fenceSequence : Nat

def jointFence {Digest : Type}
    {scheme : PolicyDigestScheme}
    (state : JointState Digest scheme) :
    CompositeFence scheme.Digest where
  sequence := state.fenceSequence
  admissionRevision := state.publication.bundle.admission.revision
  policyRevision := state.policy.revision
  policyDigest := state.policy.digest

structure JointAcknowledgementProposal {Digest : Type}
    (verifier : LifecycleVerifier Digest)
    (scheme : PolicyDigestScheme) where
  expectedFence : CompositeFence scheme.Digest
  acknowledgement : AcknowledgementProposal verifier scheme

def CanCommitJointAcknowledgement {Digest : Type}
    (verifier : LifecycleVerifier Digest)
    (scheme : PolicyDigestScheme)
    (state : JointState Digest scheme)
    (proposal : JointAcknowledgementProposal verifier scheme) : Prop :=
  proposal.expectedFence = jointFence state ∧
  CanCommitAcknowledgement
    verifier
    scheme
    state.policy
    state.publication
    proposal.acknowledgement

structure JointAcknowledgementCommit {Digest : Type}
  (verifier : LifecycleVerifier Digest)
    (scheme : PolicyDigestScheme) where
  successor : JointState Digest scheme
  acknowledgement : CommittedAcknowledgement verifier scheme
  committedFence : CompositeFence scheme.Digest

def commitJointAcknowledgement {Digest : Type}
    (verifier : LifecycleVerifier Digest)
    (scheme : PolicyDigestScheme)
    (state : JointState Digest scheme)
    (proposal : JointAcknowledgementProposal verifier scheme)
    (committable :
      CanCommitJointAcknowledgement verifier scheme state proposal) :
    JointAcknowledgementCommit verifier scheme :=
  let acknowledgement :=
    commitAcknowledgement
      verifier
      scheme
      state.policy
      state.publication
      proposal.acknowledgement
      committable.2
  let successor : JointState Digest scheme :=
    { publication :=
        { bundle := acknowledgement.closedBundle
        , sinkId := state.publication.sinkId
        , protocolVersion := state.publication.protocolVersion
        }
    , policy := state.policy
    , fenceSequence := state.fenceSequence + 1
    }
  { successor := successor
  , acknowledgement := acknowledgement
  , committedFence := jointFence successor
  }

theorem committable_joint_acknowledgement_matches_all_heads
    {Digest : Type}
    (verifier : LifecycleVerifier Digest)
    (scheme : PolicyDigestScheme)
    (state : JointState Digest scheme)
    (proposal : JointAcknowledgementProposal verifier scheme)
    (committable :
      CanCommitJointAcknowledgement verifier scheme state proposal) :
    proposal.expectedFence.sequence = state.fenceSequence ∧
    proposal.expectedFence.admissionRevision =
      state.publication.bundle.admission.revision ∧
    proposal.expectedFence.policyRevision = state.policy.revision ∧
    proposal.expectedFence.policyDigest = state.policy.digest ∧
    CanCommitAcknowledgement
      verifier scheme state.policy state.publication
      proposal.acknowledgement := by
  have fenceMatches := committable.1
  constructor
  · exact congrArg CompositeFence.sequence fenceMatches
  constructor
  · exact congrArg CompositeFence.admissionRevision fenceMatches
  constructor
  · exact congrArg CompositeFence.policyRevision fenceMatches
  constructor
  · exact congrArg CompositeFence.policyDigest fenceMatches
  · exact committable.2

theorem stale_joint_fence_cannot_commit
    {Digest : Type}
    (verifier : LifecycleVerifier Digest)
    (scheme : PolicyDigestScheme)
    (state : JointState Digest scheme)
    (proposal : JointAcknowledgementProposal verifier scheme)
    (stale :
      proposal.expectedFence.sequence < state.fenceSequence) :
    ¬ CanCommitJointAcknowledgement verifier scheme state proposal := by
  intro committable
  have equalSequence :=
    (committable_joint_acknowledgement_matches_all_heads
      verifier scheme state proposal committable).1
  exact (Nat.ne_of_lt stale) equalSequence

theorem mismatched_admission_head_cannot_commit_joint_acknowledgement
    {Digest : Type}
    (verifier : LifecycleVerifier Digest)
    (scheme : PolicyDigestScheme)
    (state : JointState Digest scheme)
    (proposal : JointAcknowledgementProposal verifier scheme)
    (mismatch :
      proposal.expectedFence.admissionRevision ≠
        state.publication.bundle.admission.revision) :
    ¬ CanCommitJointAcknowledgement verifier scheme state proposal := by
  intro committable
  exact mismatch
    (committable_joint_acknowledgement_matches_all_heads
      verifier scheme state proposal committable).2.1

theorem mismatched_policy_head_cannot_commit_joint_acknowledgement
    {Digest : Type}
    (verifier : LifecycleVerifier Digest)
    (scheme : PolicyDigestScheme)
    (state : JointState Digest scheme)
    (proposal : JointAcknowledgementProposal verifier scheme)
    (revisionMismatch :
      proposal.expectedFence.policyRevision ≠ state.policy.revision) :
    ¬ CanCommitJointAcknowledgement verifier scheme state proposal := by
  intro committable
  exact revisionMismatch
    (committable_joint_acknowledgement_matches_all_heads
      verifier scheme state proposal committable).2.2.1

theorem mismatched_policy_digest_cannot_commit_joint_acknowledgement
    {Digest : Type}
    (verifier : LifecycleVerifier Digest)
    (scheme : PolicyDigestScheme)
    (state : JointState Digest scheme)
    (proposal : JointAcknowledgementProposal verifier scheme)
    (digestMismatch :
      proposal.expectedFence.policyDigest ≠ state.policy.digest) :
    ¬ CanCommitJointAcknowledgement verifier scheme state proposal := by
  intro committable
  exact digestMismatch
    (committable_joint_acknowledgement_matches_all_heads
      verifier scheme state proposal committable).2.2.2.1

theorem joint_commit_advances_sequence_and_closes_obligation
    {Digest : Type}
    (verifier : LifecycleVerifier Digest)
    (scheme : PolicyDigestScheme)
    (state : JointState Digest scheme)
    (proposal : JointAcknowledgementProposal verifier scheme)
    (committable :
      CanCommitJointAcknowledgement verifier scheme state proposal) :
    (commitJointAcknowledgement
      verifier scheme state proposal committable).committedFence.sequence =
        state.fenceSequence + 1 ∧
    (commitJointAcknowledgement verifier scheme state proposal committable).acknowledgement.closedBundle.obligation.phase =
      PublicationPhase.acknowledged :=
  ⟨rfl, rfl⟩

theorem same_snapshot_second_joint_acknowledgement_loses
    {Digest : Type}
    (verifier : LifecycleVerifier Digest)
    (scheme : PolicyDigestScheme)
    (state : JointState Digest scheme)
    (winner loser : JointAcknowledgementProposal verifier scheme)
    (winnerAllowed :
      CanCommitJointAcknowledgement verifier scheme state winner)
    (loserObservedSameFence :
      loser.expectedFence = jointFence state) :
    ¬ CanCommitJointAcknowledgement
      verifier
      scheme
      (commitJointAcknowledgement
        verifier scheme state winner winnerAllowed).successor
      loser := by
  intro loserAllowed
  have loserSequenceAtOld :
      loser.expectedFence.sequence = state.fenceSequence :=
    congrArg CompositeFence.sequence loserObservedSameFence
  have loserSequenceAtNew :
      loser.expectedFence.sequence = state.fenceSequence + 1 :=
    (committable_joint_acknowledgement_matches_all_heads
      verifier
      scheme
      (commitJointAcknowledgement
        verifier scheme state winner winnerAllowed).successor
      loser
      loserAllowed).1
  have oldEqualsNew :
      state.fenceSequence = state.fenceSequence + 1 :=
    loserSequenceAtOld.symm.trans loserSequenceAtNew
  exact (Nat.ne_of_lt (Nat.lt_succ_self state.fenceSequence))
    oldEqualsNew

end ASPProof.SearchRouteAdmissionRetryCacheRejoinJointFence
