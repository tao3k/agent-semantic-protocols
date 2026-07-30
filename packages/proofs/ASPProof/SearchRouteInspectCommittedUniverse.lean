import ASPProof.SearchRouteInspectCertifiedLedger

namespace SearchRouteInspectCommittedUniverse

open SearchRouteCost
open SearchRouteDAG
open SearchRouteBranchBound
open SearchRouteInspectLoop
open SearchRouteInspectTraceCatalog
open SearchRouteInspectCertifiedLedger

structure CandidateCommitmentScheme where
  MembershipProof : Type
  ExclusionProof : Type
  validRoot : Nat → Prop
  contains : Nat → GraphCandidate → Prop
  verifyMembership :
    Nat → GraphCandidate → MembershipProof → Bool
  verifyExclusion :
    Nat → GraphCandidate → ExclusionProof → Bool
  membershipSound :
    ∀ root candidate proof,
      verifyMembership root candidate proof = true →
        contains root candidate
  containedRootValid :
    ∀ root candidate,
      contains root candidate →
        validRoot root
  membershipComplete :
    ∀ root candidate,
      contains root candidate →
        ∃ proof, verifyMembership root candidate proof = true
  exclusionSound :
    ∀ root candidate proof,
      verifyExclusion root candidate proof = true →
        ¬contains root candidate
  exclusionRootSound :
    ∀ root candidate proof,
      verifyExclusion root candidate proof = true →
        validRoot root
  exclusionComplete :
    ∀ root candidate,
      validRoot root →
        ¬contains root candidate →
        ∃ proof, verifyExclusion root candidate proof = true

structure MembershipOpening
    (scheme : CandidateCommitmentScheme)
    (root : Nat)
    (candidate : GraphCandidate) where
  proof : scheme.MembershipProof
  verified :
    scheme.verifyMembership root candidate proof = true

structure ExclusionOpening
    (scheme : CandidateCommitmentScheme)
    (root : Nat)
    (candidate : GraphCandidate) where
  proof : scheme.ExclusionProof
  verified :
    scheme.verifyExclusion root candidate proof = true

theorem verified_membership_is_contained
    {scheme : CandidateCommitmentScheme}
    {root : Nat}
    {candidate : GraphCandidate}
    (opening : MembershipOpening scheme root candidate) :
    scheme.contains root candidate :=
  scheme.membershipSound
    root candidate opening.proof opening.verified

theorem verified_exclusion_is_absent
    {scheme : CandidateCommitmentScheme}
    {root : Nat}
    {candidate : GraphCandidate}
    (opening : ExclusionOpening scheme root candidate) :
    ¬scheme.contains root candidate :=
  scheme.exclusionSound
    root candidate opening.proof opening.verified

theorem verified_membership_root_is_valid
    {scheme : CandidateCommitmentScheme}
    {root : Nat}
    {candidate : GraphCandidate}
    (opening : MembershipOpening scheme root candidate) :
    scheme.validRoot root :=
  scheme.containedRootValid
    root
    candidate
    (verified_membership_is_contained opening)

theorem verified_exclusion_root_is_valid
    {scheme : CandidateCommitmentScheme}
    {root : Nat}
    {candidate : GraphCandidate}
    (opening : ExclusionOpening scheme root candidate) :
    scheme.validRoot root :=
  scheme.exclusionRootSound
    root candidate opening.proof opening.verified

theorem membership_and_exclusion_cannot_both_verify
    {scheme : CandidateCommitmentScheme}
    {root : Nat}
    {candidate : GraphCandidate}
    (membership : MembershipOpening scheme root candidate)
    (exclusion : ExclusionOpening scheme root candidate) :
    False :=
  verified_exclusion_is_absent exclusion
    (verified_membership_is_contained membership)

structure CommittedCandidateUniverse
    (scheme : CandidateCommitmentScheme) where
  snapshotDigest : Nat
  rootDigest : Nat

def CommitmentComplete
    (scheme : CandidateCommitmentScheme)
    (snapshot : RankedDAGSnapshot)
    (source target : Fin snapshot.graph.nodeCount)
    (requirement : RouteRequirement)
    (budget : GraphRouteBudget)
    (candidateUniverse : CommittedCandidateUniverse scheme) :
    Prop :=
  ∀ candidate,
    Realizes snapshot.graph source target candidate →
      graphFeasibleB requirement budget candidate = true →
        scheme.contains candidateUniverse.rootDigest candidate

structure CertifiedCommittedFrontier
    (scheme : CandidateCommitmentScheme) where
  rootDigest : Nat
  lowerBound : GraphLowerBound
  boundValid :
    ∀ candidate,
      scheme.contains rootDigest candidate →
        keyLexNoWorse lowerBound (candidateKey candidate)

structure CommittedCoverageReceipt
    (scheme : CandidateCommitmentScheme)
    (candidateUniverse : CommittedCandidateUniverse scheme)
    (visible : List GraphCandidate)
    (frontiers : List (CertifiedCommittedFrontier scheme)) where
  claimedSnapshotDigest : Nat
  snapshotBound :
    claimedSnapshotDigest = candidateUniverse.snapshotDigest
  covers :
    ∀ candidate,
      scheme.contains candidateUniverse.rootDigest candidate →
        candidate ∈ visible ∨
          ∃ frontier ∈ frontiers,
            scheme.contains frontier.rootDigest candidate

def CommittedScheduleComplete
    (scheme : CandidateCommitmentScheme)
    (selected : GraphCandidate)
    (frontiers : List (CertifiedCommittedFrontier scheme)) :
    Prop :=
  ∀ frontier ∈ frontiers,
    Prunable selected frontier.lowerBound

structure CommittedTraceLedger
    (scheme : CandidateCommitmentScheme)
    (initial : InspectLoopState)
    (snapshot : RankedDAGSnapshot)
    (source target : Fin snapshot.graph.nodeCount)
    (requirement : RouteRequirement)
    (budget : GraphRouteBudget)
    (selected : GraphCandidate) where
  candidateUniverse : CommittedCandidateUniverse scheme
  universeSnapshotBound :
    candidateUniverse.snapshotDigest = snapshot.digest
  visibleCatalog :
    List
      (RealizedTraceCatalogEntry
        initial snapshot.graph source target)
  visibleFeasible :
    ∀ candidate ∈ projectTraceCatalog visibleCatalog,
      graphFeasibleB requirement budget candidate = true
  frontiers : List (CertifiedCommittedFrontier scheme)
  coverage :
    CommittedCoverageReceipt
      scheme
      candidateUniverse
      (projectTraceCatalog visibleCatalog)
      frontiers
  universeComplete :
    CommitmentComplete
      scheme
      snapshot
      source
      target
      requirement
      budget
      candidateUniverse
  selectedByRouter :
    chooseBestGraphCandidate
        requirement
        budget
        (projectTraceCatalog visibleCatalog) =
      some selected
  scheduleComplete :
    CommittedScheduleComplete scheme selected frontiers

theorem committed_ledger_snapshot_is_bound
    {scheme : CandidateCommitmentScheme}
    {initial : InspectLoopState}
    {snapshot : RankedDAGSnapshot}
    {source target : Fin snapshot.graph.nodeCount}
    {requirement : RouteRequirement}
    {budget : GraphRouteBudget}
    {selected : GraphCandidate}
    (ledger :
      CommittedTraceLedger
        scheme
        initial
        snapshot
        source
        target
        requirement
        budget
        selected) :
    ledger.coverage.claimedSnapshotDigest = snapshot.digest :=
  ledger.coverage.snapshotBound.trans ledger.universeSnapshotBound

theorem committed_ledger_visible_is_optimal
    {scheme : CandidateCommitmentScheme}
    {initial : InspectLoopState}
    {snapshot : RankedDAGSnapshot}
    {source target : Fin snapshot.graph.nodeCount}
    {requirement : RouteRequirement}
    {budget : GraphRouteBudget}
    {selected : GraphCandidate}
    (ledger :
      CommittedTraceLedger
        scheme
        initial
        snapshot
        source
        target
        requirement
        budget
        selected) :
    VisibleOptimal
      selected
      (projectTraceCatalog ledger.visibleCatalog) := by
  have selectedReceipt :=
    chooseBestGraphCandidate_mem_and_feasible
      requirement
      budget
      (projectTraceCatalog ledger.visibleCatalog)
      selected
      ledger.selectedByRouter
  refine ⟨selectedReceipt.1, ?_⟩
  intro candidate candidateVisible
  exact
    chooseBestGraphCandidate_is_lex_optimal
      requirement
      budget
      (projectTraceCatalog ledger.visibleCatalog)
      selected
      ledger.selectedByRouter
      candidate
      candidateVisible
      (ledger.visibleFeasible candidate candidateVisible)

theorem committed_frontier_pruning_is_safe
    {scheme : CandidateCommitmentScheme}
    {selected candidate : GraphCandidate}
    {frontier : CertifiedCommittedFrontier scheme}
    (prunable : Prunable selected frontier.lowerBound)
    (candidateContained :
      scheme.contains frontier.rootDigest candidate) :
    graphLexNoWorse selected candidate := by
  apply (graphLexNoWorse_iff_key selected candidate).2
  exact keyLexNoWorse_trans
    prunable
    (frontier.boundValid candidate candidateContained)

theorem committed_ledger_selected_has_provenance
    {scheme : CandidateCommitmentScheme}
    {initial : InspectLoopState}
    {snapshot : RankedDAGSnapshot}
    {source target : Fin snapshot.graph.nodeCount}
    {requirement : RouteRequirement}
    {budget : GraphRouteBudget}
    {selected : GraphCandidate}
    (ledger :
      CommittedTraceLedger
        scheme
        initial
        snapshot
        source
        target
        requirement
        budget
        selected) :
    ∃ entry ∈ ledger.visibleCatalog, entry.candidate = selected :=
  selected_candidate_has_trace_provenance ledger.selectedByRouter

theorem committed_ledger_selected_realizes
    {scheme : CandidateCommitmentScheme}
    {initial : InspectLoopState}
    {snapshot : RankedDAGSnapshot}
    {source target : Fin snapshot.graph.nodeCount}
    {requirement : RouteRequirement}
    {budget : GraphRouteBudget}
    {selected : GraphCandidate}
    (ledger :
      CommittedTraceLedger
        scheme
        initial
        snapshot
        source
        target
        requirement
        budget
        selected) :
    Realizes snapshot.graph source target selected := by
  rcases committed_ledger_selected_has_provenance ledger with
    ⟨entry, _, entryProjectsToSelected⟩
  rw [← entryProjectsToSelected]
  exact entry.realization

theorem committed_trace_ledger_selects_global_driver
    {scheme : CandidateCommitmentScheme}
    {initial : InspectLoopState}
    {snapshot : RankedDAGSnapshot}
    {source target : Fin snapshot.graph.nodeCount}
    {requirement : RouteRequirement}
    {budget : GraphRouteBudget}
    {selected : GraphCandidate}
    (ledger :
      CommittedTraceLedger
        scheme
        initial
        snapshot
        source
        target
        requirement
        budget
        selected) :
    (∃ entry ∈ ledger.visibleCatalog, entry.candidate = selected) ∧
      Realizes snapshot.graph source target selected ∧
      ∀ candidate,
        Realizes snapshot.graph source target candidate →
          graphFeasibleB requirement budget candidate = true →
            graphLexNoWorse selected candidate := by
  refine ⟨
    committed_ledger_selected_has_provenance ledger,
    committed_ledger_selected_realizes ledger,
    ?_
  ⟩
  intro candidate candidateRealizes candidateFeasible
  have candidateInUniverse :
      scheme.contains ledger.candidateUniverse.rootDigest candidate :=
    ledger.universeComplete
      candidate
      candidateRealizes
      candidateFeasible
  rcases ledger.coverage.covers candidate candidateInUniverse with
    candidateVisible | candidateDeferred
  · exact
      (committed_ledger_visible_is_optimal ledger).2
        candidate
        candidateVisible
  · rcases candidateDeferred with
      ⟨frontier, frontierMember, candidateContained⟩
    exact
      committed_frontier_pruning_is_safe
        (ledger.scheduleComplete frontier frontierMember)
        candidateContained

structure DigestOnlyUniverse where
  rootDigest : Nat
  contains : GraphCandidate → Prop

theorem digest_only_universe_is_not_binding
    (candidate : GraphCandidate) :
    ∃ left right : DigestOnlyUniverse,
      left.rootDigest = right.rootDigest ∧
        left.contains candidate ∧
          ¬right.contains candidate := by
  refine ⟨
    { rootDigest := 0, contains := fun _ => True },
    { rootDigest := 0, contains := fun _ => False },
    rfl,
    ?_,
    ?_
  ⟩
  · trivial
  · simp

end SearchRouteInspectCommittedUniverse
