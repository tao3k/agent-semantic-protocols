import ASPProof.SearchRouteDAG

namespace SearchRouteGraphCostVector

open SearchRouteDAG

structure GraphCostVector where
  graphHops : Nat
  uncachedTokens : Nat
  rounds : Nat
  transitions : Nat
  deriving DecidableEq, Repr

def ofCandidate (candidate : GraphCandidate) : GraphCostVector := {
  graphHops := candidate.graphHops
  uncachedTokens :=
    SearchRouteCost.uncachedTokenCost candidate.route
  rounds :=
    SearchRouteCost.roundCost candidate.route
  transitions :=
    SearchRouteCost.transitionCount candidate.route
}

def RoundTransitionNoWorse
    (leftRounds leftTransitions rightRounds rightTransitions : Nat) : Prop :=
  leftRounds < rightRounds ∨
  leftRounds = rightRounds ∧
    leftTransitions ≤ rightTransitions

def TokenTailNoWorse
    (leftTokens leftRounds leftTransitions
      rightTokens rightRounds rightTransitions : Nat) : Prop :=
  leftTokens < rightTokens ∨
  leftTokens = rightTokens ∧
    RoundTransitionNoWorse
      leftRounds leftTransitions
      rightRounds rightTransitions

def CostVectorNoWorse
    (left right : GraphCostVector) : Prop :=
  left.graphHops < right.graphHops ∨
  left.graphHops = right.graphHops ∧
    TokenTailNoWorse
      left.uncachedTokens left.rounds left.transitions
      right.uncachedTokens right.rounds right.transitions

theorem round_transition_refl
    (rounds transitions : Nat) :
    RoundTransitionNoWorse rounds transitions rounds transitions := by
  simp [RoundTransitionNoWorse]

theorem round_transition_trans
    {aRounds aTransitions
      bRounds bTransitions
      cRounds cTransitions : Nat}
    (ab :
      RoundTransitionNoWorse
        aRounds aTransitions bRounds bTransitions)
    (bc :
      RoundTransitionNoWorse
        bRounds bTransitions cRounds cTransitions) :
    RoundTransitionNoWorse
      aRounds aTransitions cRounds cTransitions := by
  unfold RoundTransitionNoWorse at *
  omega

theorem token_tail_refl
    (tokens rounds transitions : Nat) :
    TokenTailNoWorse
      tokens rounds transitions
      tokens rounds transitions := by
  simp [TokenTailNoWorse, round_transition_refl]

theorem token_tail_trans
    {aTokens aRounds aTransitions
      bTokens bRounds bTransitions
      cTokens cRounds cTransitions : Nat}
    (ab :
      TokenTailNoWorse
        aTokens aRounds aTransitions
        bTokens bRounds bTransitions)
    (bc :
      TokenTailNoWorse
        bTokens bRounds bTransitions
        cTokens cRounds cTransitions) :
    TokenTailNoWorse
      aTokens aRounds aTransitions
      cTokens cRounds cTransitions := by
  unfold TokenTailNoWorse at *
  rcases ab with tokenBetter | ⟨tokenEqual, tailNoWorse⟩
  · rcases bc with nextTokenBetter | ⟨nextTokenEqual, _⟩
    · exact Or.inl (Nat.lt_trans tokenBetter nextTokenBetter)
    · exact Or.inl (by simpa [nextTokenEqual] using tokenBetter)
  · rcases bc with nextTokenBetter | ⟨nextTokenEqual, nextTailNoWorse⟩
    · exact Or.inl (by simpa [tokenEqual] using nextTokenBetter)
    · refine Or.inr ⟨tokenEqual.trans nextTokenEqual, ?_⟩
      exact round_transition_trans tailNoWorse nextTailNoWorse

theorem cost_vector_refl
    (cost : GraphCostVector) :
    CostVectorNoWorse cost cost := by
  simp [CostVectorNoWorse, token_tail_refl]

theorem cost_vector_trans
    {a b c : GraphCostVector}
    (ab : CostVectorNoWorse a b)
    (bc : CostVectorNoWorse b c) :
    CostVectorNoWorse a c := by
  unfold CostVectorNoWorse at *
  rcases ab with hopBetter | ⟨hopEqual, tailNoWorse⟩
  · rcases bc with nextHopBetter | ⟨nextHopEqual, _⟩
    · exact Or.inl (Nat.lt_trans hopBetter nextHopBetter)
    · exact Or.inl (by simpa [nextHopEqual] using hopBetter)
  · rcases bc with nextHopBetter | ⟨nextHopEqual, nextTailNoWorse⟩
    · exact Or.inl (by simpa [hopEqual] using nextHopBetter)
    · refine Or.inr ⟨hopEqual.trans nextHopEqual, ?_⟩
      exact token_tail_trans tailNoWorse nextTailNoWorse

theorem candidate_order_equivalent
    (left right : GraphCandidate) :
    CostVectorNoWorse (ofCandidate left) (ofCandidate right) ↔
      graphLexNoWorse left right :=
  Iff.rfl

structure CertifiedCostFrontier where
  candidates : List GraphCandidate
  lowerBound : GraphCostVector
  lowerBoundValid :
    ∀ candidate,
      candidate ∈ candidates →
      CostVectorNoWorse lowerBound (ofCandidate candidate)

def VectorSparseCoverage
    (fullCatalog visible : List GraphCandidate)
    (frontiers : List CertifiedCostFrontier) : Prop :=
  ∀ candidate,
    candidate ∈ fullCatalog →
    candidate ∈ visible ∨
      ∃ frontier,
        frontier ∈ frontiers ∧
        candidate ∈ frontier.candidates

def VectorVisibleOptimal
    (selected : GraphCandidate)
    (visible : List GraphCandidate) : Prop :=
  ∀ candidate,
    candidate ∈ visible →
    CostVectorNoWorse
      (ofCandidate selected)
      (ofCandidate candidate)

def VectorFrontiersClosed
    (selected : GraphCandidate)
    (frontiers : List CertifiedCostFrontier) : Prop :=
  ∀ frontier,
    frontier ∈ frontiers →
    CostVectorNoWorse
      (ofCandidate selected)
      frontier.lowerBound

def VectorGlobalOptimal
    (selected : GraphCandidate)
    (fullCatalog : List GraphCandidate) : Prop :=
  ∀ candidate,
    candidate ∈ fullCatalog →
    CostVectorNoWorse
      (ofCandidate selected)
      (ofCandidate candidate)

theorem certified_cost_frontier_selection_is_global
    {selected : GraphCandidate}
    {fullCatalog visible : List GraphCandidate}
    {frontiers : List CertifiedCostFrontier}
    (visibleOptimal : VectorVisibleOptimal selected visible)
    (coverage : VectorSparseCoverage fullCatalog visible frontiers)
    (closed : VectorFrontiersClosed selected frontiers) :
    VectorGlobalOptimal selected fullCatalog := by
  intro candidate inFull
  rcases coverage candidate inFull with inVisible | ⟨frontier, frontierMem, candidateMem⟩
  · exact visibleOptimal candidate inVisible
  · exact cost_vector_trans
      (closed frontier frontierMem)
      (frontier.lowerBoundValid candidate candidateMem)

theorem vector_global_optimality_implies_graph_global_optimality
    {selected : GraphCandidate}
    {fullCatalog : List GraphCandidate}
    (vectorGlobal : VectorGlobalOptimal selected fullCatalog) :
    ∀ candidate,
      candidate ∈ fullCatalog →
      graphLexNoWorse selected candidate := by
  intro candidate inFull
  exact (candidate_order_equivalent selected candidate).1
    (vectorGlobal candidate inFull)

def expensiveOneHop : GraphCostVector := {
  graphHops := 1
  uncachedTokens := 100
  rounds := 1
  transitions := 1
}

def cheapOneHop : GraphCostVector := {
  graphHops := 1
  uncachedTokens := 10
  rounds := 1
  transitions := 1
}

theorem hop_bound_alone_does_not_close_frontier :
    expensiveOneHop.graphHops ≤ cheapOneHop.graphHops ∧
    ¬ CostVectorNoWorse expensiveOneHop cheapOneHop := by
  decide

end SearchRouteGraphCostVector
