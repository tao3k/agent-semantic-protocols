import ASPProof.SearchRouteAdmissionRetryCacheRejoinReplayProtection

namespace ASPProof.SearchRouteAdmissionRetryCacheRejoinLinearization

open ASPProof.SearchRouteAdmissionRetryCacheRejoinReplayProtection

inductive AdmissionMutation (Digest : Type) where
  | activate (claim : AdmissionClaim Digest)
  | beginAttempt (nextEpoch : Nat)

def MutationAllowed {Digest : Type}
    (state : ReplicaAdmissionState Digest) :
    AdmissionMutation Digest → Prop
  | .activate claim => CanActivate state claim
  | .beginAttempt nextEpoch => CanBeginNewAttempt state nextEpoch

def applyMutation {Digest : Type}
    (state : ReplicaAdmissionState Digest) :
    AdmissionMutation Digest → ReplicaAdmissionState Digest
  | .activate claim => activate state claim
  | .beginAttempt nextEpoch => beginNewAttempt state nextEpoch

structure RevisionedAdmissionState (Digest : Type) where
  state : ReplicaAdmissionState Digest
  revision : Nat

structure MutationProposal (Digest : Type) where
  expectedRevision : Nat
  mutation : AdmissionMutation Digest

def CanCommit {Digest : Type}
    (current : RevisionedAdmissionState Digest)
    (proposal : MutationProposal Digest) : Prop :=
  proposal.expectedRevision = current.revision ∧
  MutationAllowed current.state proposal.mutation

def commit {Digest : Type}
    (current : RevisionedAdmissionState Digest)
    (proposal : MutationProposal Digest)
    (_ : CanCommit current proposal) :
    RevisionedAdmissionState Digest where
  state := applyMutation current.state proposal.mutation
  revision := current.revision + 1

def activationProposal {Digest : Type}
    (expectedRevision : Nat)
    (claim : AdmissionClaim Digest) :
    MutationProposal Digest where
  expectedRevision := expectedRevision
  mutation := .activate claim

def beginAttemptProposal {Digest : Type}
    (expectedRevision nextEpoch : Nat) :
    MutationProposal Digest where
  expectedRevision := expectedRevision
  mutation := .beginAttempt nextEpoch

theorem committable_proposal_has_exact_revision_and_allowed_mutation
    {Digest : Type}
    (current : RevisionedAdmissionState Digest)
    (proposal : MutationProposal Digest)
    (committable : CanCommit current proposal) :
    proposal.expectedRevision = current.revision ∧
    MutationAllowed current.state proposal.mutation :=
  committable

theorem committed_mutation_advances_revision_once
    {Digest : Type}
    (current : RevisionedAdmissionState Digest)
    (proposal : MutationProposal Digest)
    (allowed : CanCommit current proposal) :
    (commit current proposal allowed).revision = current.revision + 1 :=
  rfl

theorem stale_revision_proposal_cannot_commit
    {Digest : Type}
    (current : RevisionedAdmissionState Digest)
    (proposal : MutationProposal Digest)
    (stale : proposal.expectedRevision < current.revision) :
    ¬ CanCommit current proposal := by
  intro committable
  exact (Nat.ne_of_lt stale) committable.1

theorem same_snapshot_second_proposal_loses_revision_race
    {Digest : Type}
    (current : RevisionedAdmissionState Digest)
    (winner loser : MutationProposal Digest)
    (winnerAllowed : CanCommit current winner)
    (loserObservedSameRevision :
      loser.expectedRevision = current.revision) :
    ¬ CanCommit (commit current winner winnerAllowed) loser := by
  intro loserAllowed
  have oldEqualsSuccessor :
      current.revision = current.revision + 1 :=
    loserObservedSameRevision.symm.trans loserAllowed.1
  exact (Nat.ne_of_lt (Nat.lt_succ_self current.revision))
    oldEqualsSuccessor

theorem concurrent_activation_and_epoch_advance_have_one_revision_winner
    {Digest : Type}
    (current : RevisionedAdmissionState Digest)
    (claim : AdmissionClaim Digest)
    (nextEpoch : Nat)
    (activationAllowed :
      CanCommit current
        (activationProposal current.revision claim)) :
    ¬ CanCommit
      (commit current
        (activationProposal current.revision claim)
        activationAllowed)
      (beginAttemptProposal current.revision nextEpoch) :=
  same_snapshot_second_proposal_loses_revision_race
    current
    (activationProposal current.revision claim)
    (beginAttemptProposal current.revision nextEpoch)
    activationAllowed
    rfl

structure PublishedAdmissionEdge (Digest : Type) where
  claim : AdmissionClaim Digest
  committedRevision : Nat

def EdgeAuthorized {Digest : Type}
    (current : RevisionedAdmissionState Digest)
    (edge : PublishedAdmissionEdge Digest) : Prop :=
  current.revision = edge.committedRevision ∧
  current.state.active = some edge.claim

def activationEdge {Digest : Type}
    (current : RevisionedAdmissionState Digest)
    (claim : AdmissionClaim Digest) :
    PublishedAdmissionEdge Digest where
  claim := claim
  committedRevision := current.revision + 1

theorem activation_commit_authorizes_bound_graph_edge
    {Digest : Type}
    (current : RevisionedAdmissionState Digest)
    (claim : AdmissionClaim Digest)
    (allowed :
      CanCommit current
        (activationProposal current.revision claim)) :
    EdgeAuthorized
      (commit current
        (activationProposal current.revision claim)
        allowed)
      (activationEdge current claim) := by
  constructor <;> rfl

theorem conflicting_claim_cannot_share_winner_edge
    {Digest : Type}
    (current : RevisionedAdmissionState Digest)
    (winner conflicting : AdmissionClaim Digest)
    (allowed :
      CanCommit current
        (activationProposal current.revision winner))
    (differentTerminal :
      winner.terminalDigest ≠ conflicting.terminalDigest) :
    ¬ EdgeAuthorized
      (commit current
        (activationProposal current.revision winner)
        allowed)
      { claim := conflicting
      , committedRevision := current.revision + 1
      } := by
  intro authorized
  have winnerActive :
      (commit current
        (activationProposal current.revision winner)
        allowed).state.active = some winner :=
    rfl
  have sameClaim : winner = conflicting :=
    Option.some.inj (winnerActive.symm.trans authorized.2)
  exact differentTerminal
    (congrArg AdmissionClaim.terminalDigest sameClaim)

theorem stale_revision_edge_is_not_authorized
    {Digest : Type}
    (current : RevisionedAdmissionState Digest)
    (edge : PublishedAdmissionEdge Digest)
    (stale : edge.committedRevision < current.revision) :
    ¬ EdgeAuthorized current edge := by
  intro authorized
  exact (Nat.ne_of_lt stale) authorized.1.symm

end ASPProof.SearchRouteAdmissionRetryCacheRejoinLinearization
