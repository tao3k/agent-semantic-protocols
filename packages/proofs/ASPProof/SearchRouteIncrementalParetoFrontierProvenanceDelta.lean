import ASPProof.SearchRouteMultiobjectivePathCostFrontierTermination

namespace ASPProof.SearchRouteIncrementalParetoFrontierProvenanceDelta

open ASPProof.SearchRouteGraphRouterParetoCostSelection

structure FrontierEntry where
  canonicalRouteIdentity : Nat
  evidenceProvenanceDigest : Nat
  cost : RouteCost

def CostEquivalent
    (left right : RouteCost) : Prop :=
  NoWorse left right ∧ NoWorse right left

def SameCanonicalRoute
    (left right : FrontierEntry) : Prop :=
  left.canonicalRouteIdentity = right.canonicalRouteIdentity

def StrictFrontierAntichain
    (Frontier : FrontierEntry → Prop) : Prop :=
  ∀ left right,
    Frontier left →
    Frontier right →
    left.canonicalRouteIdentity
      ≠ right.canonicalRouteIdentity →
    (¬ StrictlyDominates left.cost right.cost)
      ∧ (¬ StrictlyDominates right.cost left.cost)

def EntryParetoCovers
    (Frontier Candidates : FrontierEntry → Prop) : Prop :=
  ∀ candidate,
    Candidates candidate →
    ∃ frontierEntry,
      Frontier frontierEntry
        ∧ NoWorse frontierEntry.cost candidate.cost

def CandidateUndominated
    (Frontier : FrontierEntry → Prop)
    (candidate : FrontierEntry) : Prop :=
  ∀ existing,
    Frontier existing →
    ¬ StrictlyDominates existing.cost candidate.cost

def CandidateFresh
    (Frontier : FrontierEntry → Prop)
    (candidate : FrontierEntry) : Prop :=
  ∀ existing,
    Frontier existing →
    existing.canonicalRouteIdentity
      ≠ candidate.canonicalRouteIdentity

def InsertedFrontier
    (Frontier : FrontierEntry → Prop)
    (candidate : FrontierEntry)
    (entry : FrontierEntry) : Prop :=
  entry = candidate
    ∨ (Frontier entry
      ∧ ¬ StrictlyDominates candidate.cost entry.cost)

def CandidatesAfterInsert
    (Candidates : FrontierEntry → Prop)
    (candidate entry : FrontierEntry) : Prop :=
  entry = candidate ∨ Candidates entry

def ExplicitDecisionDeltaBytes
    (fixedBytes candidateBytes removalCount
      removalReceiptByteMaximum : Nat) : Nat :=
  fixedBytes
    + candidateBytes
    + removalCount * removalReceiptByteMaximum

def SummarizedDecisionDeltaBytes
    (fixedBytes candidateBytes _removalCount
      removalSummaryBytes : Nat) : Nat :=
  fixedBytes + candidateBytes + removalSummaryBytes

theorem strict_dominance_excludes_reverse_no_worse
    (left right : RouteCost)
    (dominates : StrictlyDominates left right)
    (reverseBound : NoWorse right left) :
    False := by
  rcases dominates.2 with
    graphHopsBetter
    | interactionRoundsBetter
    | projectedTokensBetter
    | verificationOpsBetter
    | searchExecutionsBetter
    | modelPrefixRecomputationsBetter
  · exact Nat.not_lt_of_ge reverseBound.1 graphHopsBetter
  · exact Nat.not_lt_of_ge
      reverseBound.2.1
      interactionRoundsBetter
  · exact Nat.not_lt_of_ge
      reverseBound.2.2.1
      projectedTokensBetter
  · exact Nat.not_lt_of_ge
      reverseBound.2.2.2.1
      verificationOpsBetter
  · exact Nat.not_lt_of_ge
      reverseBound.2.2.2.2.1
      searchExecutionsBetter
  · exact Nat.not_lt_of_ge
      reverseBound.2.2.2.2.2
      modelPrefixRecomputationsBetter

theorem cost_equivalence_excludes_strict_dominance
    (left right : RouteCost)
    (equivalent : CostEquivalent left right) :
    (¬ StrictlyDominates left right)
      ∧ (¬ StrictlyDominates right left) := by
  constructor
  · intro dominates
    exact strict_dominance_excludes_reverse_no_worse
      left
      right
      dominates
      equivalent.2
  · intro dominates
    exact strict_dominance_excludes_reverse_no_worse
      right
      left
      dominates
      equivalent.1

theorem undominated_fresh_insertion_preserves_strict_antichain
    (Frontier : FrontierEntry → Prop)
    (candidate : FrontierEntry)
    (oldAntichain : StrictFrontierAntichain Frontier)
    (undominated : CandidateUndominated Frontier candidate)
    (_fresh : CandidateFresh Frontier candidate) :
    StrictFrontierAntichain
      (InsertedFrontier Frontier candidate) := by
  intro left right leftMember rightMember different
  rcases leftMember with leftCandidate | ⟨leftOld, leftSurvives⟩
  · rcases rightMember with rightCandidate | ⟨rightOld, rightSurvives⟩
    · rw [leftCandidate, rightCandidate] at different
      exact (different rfl).elim
    · rw [leftCandidate]
      exact ⟨rightSurvives, undominated right rightOld⟩
  · rcases rightMember with rightCandidate | ⟨rightOld, rightSurvives⟩
    · rw [rightCandidate]
      exact ⟨undominated left leftOld, leftSurvives⟩
    · exact oldAntichain
        left
        right
        leftOld
        rightOld
        different

theorem insertion_preserves_pareto_coverage
    (Frontier Candidates : FrontierEntry → Prop)
    (candidate : FrontierEntry)
    (oldCoverage : EntryParetoCovers Frontier Candidates)
    (dominanceDecision :
      ∀ existing,
        Frontier existing →
        StrictlyDominates candidate.cost existing.cost
          ∨ ¬ StrictlyDominates candidate.cost existing.cost) :
    EntryParetoCovers
      (InsertedFrontier Frontier candidate)
      (CandidatesAfterInsert Candidates candidate) := by
  intro considered consideredMember
  rcases consideredMember with consideredCandidate | consideredOld
  · refine ⟨candidate, Or.inl rfl, ?_⟩
    rw [consideredCandidate]
    exact no_worse_is_reflexive candidate.cost
  · obtain ⟨existing, existingMember, existingBound⟩ :=
      oldCoverage considered consideredOld
    rcases dominanceDecision existing existingMember with
      candidateDominates | candidateDoesNotDominate
    · refine ⟨candidate, Or.inl rfl, ?_⟩
      exact no_worse_is_transitive
        candidate.cost
        existing.cost
        considered.cost
        candidateDominates.1
        existingBound
    · exact ⟨
        existing,
        Or.inr ⟨existingMember, candidateDoesNotDominate⟩,
        existingBound⟩

theorem equal_cost_distinct_provenance_routes_survive_insertion
    (Frontier : FrontierEntry → Prop)
    (existing candidate : FrontierEntry)
    (existingMember : Frontier existing)
    (equivalent : CostEquivalent existing.cost candidate.cost) :
    InsertedFrontier Frontier candidate candidate
      ∧ InsertedFrontier Frontier candidate existing := by
  have notDominated :=
    (cost_equivalence_excludes_strict_dominance
      candidate.cost
      existing.cost
      ⟨equivalent.2, equivalent.1⟩).1
  exact ⟨
    Or.inl rfl,
    Or.inr ⟨existingMember, notDominated⟩⟩

theorem equal_cost_routes_can_have_distinct_canonical_identity :
    let sharedCost : RouteCost :=
      ⟨1, 0, 0, 0, 0, 0⟩
    let left : FrontierEntry :=
      ⟨1, 11, sharedCost⟩
    let right : FrontierEntry :=
      ⟨2, 22, sharedCost⟩
    CostEquivalent left.cost right.cost
      ∧ left.canonicalRouteIdentity
        < right.canonicalRouteIdentity := by
  dsimp
  exact ⟨
    ⟨
      no_worse_is_reflexive ⟨1, 0, 0, 0, 0, 0⟩,
      no_worse_is_reflexive ⟨1, 0, 0, 0, 0, 0⟩⟩,
    Nat.lt_succ_self 1⟩

theorem canonical_identity_alone_can_hide_cost_drift :
    let cheaper : FrontierEntry :=
      ⟨7, 70, ⟨1, 0, 0, 0, 0, 0⟩⟩
    let drifted : FrontierEntry :=
      ⟨7, 71, ⟨2, 0, 0, 0, 0, 0⟩⟩
    SameCanonicalRoute cheaper drifted
      ∧ StrictlyDominates cheaper.cost drifted.cost := by
  dsimp
  constructor
  · rfl
  · constructor
    · exact ⟨
        Nat.le_succ 1,
        Nat.le_refl 0,
        Nat.le_refl 0,
        Nat.le_refl 0,
        Nat.le_refl 0,
        Nat.le_refl 0⟩
    · exact Or.inl (Nat.lt_succ_self 1)

theorem explicit_delta_is_bounded_by_frontier_capacity
    (fixedBytes candidateBytes removalCount frontierCapacity
      removalReceiptByteMaximum : Nat)
    (removalBound : removalCount ≤ frontierCapacity) :
    ExplicitDecisionDeltaBytes
        fixedBytes
        candidateBytes
        removalCount
        removalReceiptByteMaximum
      ≤
    ExplicitDecisionDeltaBytes
        fixedBytes
        candidateBytes
        frontierCapacity
        removalReceiptByteMaximum := by
  unfold ExplicitDecisionDeltaBytes
  exact Nat.add_le_add_left
    (Nat.mul_le_mul_right
      removalReceiptByteMaximum
      removalBound)
    (fixedBytes + candidateBytes)

theorem uncapped_explicit_removals_have_no_fixed_delta_bound
    (byteBound : Nat) :
    byteBound
      < ExplicitDecisionDeltaBytes
        0
        0
        (Nat.succ byteBound)
        1 := by
  unfold ExplicitDecisionDeltaBytes
  rw [Nat.zero_add, Nat.mul_one]
  exact Nat.lt_succ_self byteBound

theorem summarized_delta_is_independent_of_removal_count
    (fixedBytes candidateBytes firstRemovalCount secondRemovalCount
      removalSummaryBytes : Nat) :
    SummarizedDecisionDeltaBytes
        fixedBytes
        candidateBytes
        firstRemovalCount
        removalSummaryBytes
      =
    SummarizedDecisionDeltaBytes
        fixedBytes
        candidateBytes
        secondRemovalCount
        removalSummaryBytes := by
  rfl

end ASPProof.SearchRouteIncrementalParetoFrontierProvenanceDelta
