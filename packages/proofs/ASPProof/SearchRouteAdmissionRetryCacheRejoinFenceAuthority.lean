import ASPProof.SearchRouteAdmissionRetryCacheRejoinJointFence

namespace ASPProof.SearchRouteAdmissionRetryCacheRejoinFenceAuthority

open ASPProof.SearchRouteAdmissionRetryCacheRejoinJointFence
open ASPProof.SearchRouteAdmissionRetryCacheRejoinPolicySnapshot
open ASPProof.SearchRouteAdmissionRetryCacheRejoinVerifierLifecycle

abbrev ConsensusTerm := Nat
abbrev LeaderId := Nat

structure FenceCommitReceipt (PolicyDigest : Type) where
  term : ConsensusTerm
  leaderId : LeaderId
  observedFence : CompositeFence PolicyDigest
  committedFence : CompositeFence PolicyDigest

structure FenceAuthorityState (Digest : Type)
    (scheme : PolicyDigestScheme) where
  joint : JointState Digest scheme
  term : ConsensusTerm
  leaderId : LeaderId
  receiptLedger : List (FenceCommitReceipt scheme.Digest)

structure AuthorityCommand {Digest : Type}
    (verifier : LifecycleVerifier Digest)
    (scheme : PolicyDigestScheme) where
  term : ConsensusTerm
  leaderId : LeaderId
  proposal : JointAcknowledgementProposal verifier scheme

def CanCommitAuthorityCommand {Digest : Type}
    (verifier : LifecycleVerifier Digest)
    (scheme : PolicyDigestScheme)
    (state : FenceAuthorityState Digest scheme)
    (command : AuthorityCommand verifier scheme) : Prop :=
  command.term = state.term ∧
  command.leaderId = state.leaderId ∧
  CanCommitJointAcknowledgement
    verifier scheme state.joint command.proposal

def commitAuthorityCommand {Digest : Type}
    (verifier : LifecycleVerifier Digest)
    (scheme : PolicyDigestScheme)
    (state : FenceAuthorityState Digest scheme)
    (command : AuthorityCommand verifier scheme)
    (committable :
      CanCommitAuthorityCommand verifier scheme state command) :
    FenceAuthorityState Digest scheme :=
  let jointCommit :=
    commitJointAcknowledgement
      verifier scheme state.joint command.proposal committable.2.2
  let receipt : FenceCommitReceipt scheme.Digest :=
    { term := state.term
    , leaderId := state.leaderId
    , observedFence := jointFence state.joint
    , committedFence := jointCommit.committedFence
    }
  { joint := jointCommit.successor
  , term := state.term
  , leaderId := state.leaderId
  , receiptLedger := receipt :: state.receiptLedger
  }

def electLeader {Digest : Type}
    (state : FenceAuthorityState Digest scheme)
    (newLeader : LeaderId) :
    FenceAuthorityState Digest scheme :=
  { state with term := state.term + 1, leaderId := newLeader }

theorem authority_commit_records_bound_durable_receipt
    {Digest : Type}
    (verifier : LifecycleVerifier Digest)
    (scheme : PolicyDigestScheme)
    (state : FenceAuthorityState Digest scheme)
    (command : AuthorityCommand verifier scheme)
    (committable :
      CanCommitAuthorityCommand verifier scheme state command) :
    ∃ receipt,
      (commitAuthorityCommand
        verifier scheme state command committable).receiptLedger =
          receipt :: state.receiptLedger ∧
      receipt.term = state.term ∧
      receipt.leaderId = state.leaderId ∧
      receipt.observedFence = jointFence state.joint ∧
      receipt.committedFence =
        jointFence
          (commitAuthorityCommand
            verifier scheme state command committable).joint := by
  refine ⟨_, rfl, rfl, rfl, rfl, ?_⟩
  rfl

theorem stale_term_command_cannot_commit
    {Digest : Type}
    (verifier : LifecycleVerifier Digest)
    (scheme : PolicyDigestScheme)
    (state : FenceAuthorityState Digest scheme)
    (command : AuthorityCommand verifier scheme)
    (stale : command.term < state.term) :
    ¬ CanCommitAuthorityCommand verifier scheme state command := by
  intro committable
  exact (Nat.ne_of_lt stale) committable.1

theorem nonleader_command_cannot_commit
    {Digest : Type}
    (verifier : LifecycleVerifier Digest)
    (scheme : PolicyDigestScheme)
    (state : FenceAuthorityState Digest scheme)
    (command : AuthorityCommand verifier scheme)
    (notLeader : command.leaderId ≠ state.leaderId) :
    ¬ CanCommitAuthorityCommand verifier scheme state command := by
  intro committable
  exact notLeader committable.2.1

theorem leader_change_fences_old_term
    {Digest : Type}
    (verifier : LifecycleVerifier Digest)
    (scheme : PolicyDigestScheme)
    (state : FenceAuthorityState Digest scheme)
    (oldCommand : AuthorityCommand verifier scheme)
    (newLeader : LeaderId)
    (observedOldTerm : oldCommand.term = state.term) :
    ¬ CanCommitAuthorityCommand
      verifier scheme (electLeader state newLeader) oldCommand := by
  intro committable
  have oldEqualsNew :
      state.term = state.term + 1 :=
    observedOldTerm.symm.trans committable.1
  exact (Nat.ne_of_lt (Nat.lt_succ_self state.term)) oldEqualsNew

theorem committed_receipt_is_queryable_after_lost_response
    {Digest : Type}
    (verifier : LifecycleVerifier Digest)
    (scheme : PolicyDigestScheme)
    (state : FenceAuthorityState Digest scheme)
    (command : AuthorityCommand verifier scheme)
    (committable :
      CanCommitAuthorityCommand verifier scheme state command) :
    ∃ receipt,
      receipt ∈
        (commitAuthorityCommand
          verifier scheme state command committable).receiptLedger ∧
      receipt.observedFence = jointFence state.joint := by
  rcases authority_commit_records_bound_durable_receipt
      verifier scheme state command committable with
    ⟨receipt, ledger, _, _, observed, _⟩
  refine ⟨receipt, ?_, observed⟩
  rw [ledger]
  simp

theorem later_commit_preserves_existing_receipt
    {Digest : Type}
    (verifier : LifecycleVerifier Digest)
    (scheme : PolicyDigestScheme)
    (state : FenceAuthorityState Digest scheme)
    (command : AuthorityCommand verifier scheme)
    (committable :
      CanCommitAuthorityCommand verifier scheme state command)
    (existing : FenceCommitReceipt scheme.Digest)
    (present : existing ∈ state.receiptLedger) :
    existing ∈
      (commitAuthorityCommand
        verifier scheme state command committable).receiptLedger := by
  rcases authority_commit_records_bound_durable_receipt
      verifier scheme state command committable with
    ⟨receipt, ledger, _, _, _, _⟩
  rw [ledger]
  simp [present]

end ASPProof.SearchRouteAdmissionRetryCacheRejoinFenceAuthority
