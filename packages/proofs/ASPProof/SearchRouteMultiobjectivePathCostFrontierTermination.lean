import ASPProof.SearchRouteEvidenceGraphShortestPathFrontierCompleteness

namespace ASPProof.SearchRouteMultiobjectivePathCostFrontierTermination

open ASPProof.SearchRouteGraphRouterParetoCostSelection
open ASPProof.SearchRouteEvidenceGraphShortestPathFrontierCompleteness

structure PathLocalCost where
  encodedReceiptBytes : Nat
  verificationOps : Nat

structure ExecutionScheduleCost where
  interactionRounds : Nat
  projectedTokens : Nat
  searchExecutions : Nat
  modelPrefixRecomputations : Nat

def ZeroPathLocalCost : PathLocalCost :=
  ⟨0, 0⟩

def AddPathLocalCost
    (left right : PathLocalCost) : PathLocalCost :=
  ⟨
    left.encodedReceiptBytes + right.encodedReceiptBytes,
    left.verificationOps + right.verificationOps⟩

def PathLocalCostOf
    {Edge : Nat → Nat → Prop}
    (edgeCost :
      ∀ {source target},
        Edge source target → PathLocalCost)
    {source target : Nat} :
    EvidencePath Edge source target → PathLocalCost
  | .stay _ => ZeroPathLocalCost
  | .step edge tail =>
      AddPathLocalCost
        (PathLocalCostOf edgeCost tail)
        (edgeCost edge)

def AssembleRouteCostFromParts
    (graphHops : Nat)
    (pathLocal : PathLocalCost)
    (schedule : ExecutionScheduleCost) : RouteCost :=
  {
    graphHops := graphHops
    interactionRounds := schedule.interactionRounds
    projectedTokens := schedule.projectedTokens
    verificationOps := pathLocal.verificationOps
    searchExecutions := schedule.searchExecutions
    modelPrefixRecomputations := schedule.modelPrefixRecomputations
  }

def AssemblePathRouteCost
    {Edge : Nat → Nat → Prop}
    {source target : Nat}
    (path : EvidencePath Edge source target)
    (pathLocal : PathLocalCost)
    (schedule : ExecutionScheduleCost) : RouteCost :=
  AssembleRouteCostFromParts path.hopCount pathLocal schedule

def ExecutionNoWorse
    (left right : ExecutionScheduleCost) : Prop :=
  left.interactionRounds ≤ right.interactionRounds
    ∧ left.projectedTokens ≤ right.projectedTokens
    ∧ left.searchExecutions ≤ right.searchExecutions
    ∧ left.modelPrefixRecomputations
      ≤ right.modelPrefixRecomputations

def LocalNoWorse
    (left right : PathLocalCost) : Prop :=
  left.encodedReceiptBytes ≤ right.encodedReceiptBytes
    ∧ left.verificationOps ≤ right.verificationOps

def SaturatingTokenCount : Nat → Nat
  | 0 => 0
  | Nat.succ _ => 1

def FrontierAntichain
    (Frontier : RouteCost → Prop) : Prop :=
  ∀ left right,
    Frontier left →
    Frontier right →
    left ≠ right →
    ParetoIncomparable left right

def ParetoCovers
    (Frontier Candidates : RouteCost → Prop) : Prop :=
  ∀ candidate,
    Candidates candidate →
    ∃ frontierCost,
      Frontier frontierCost
        ∧ NoWorse frontierCost candidate

def CheapRoute : RouteCost :=
  ⟨1, 0, 0, 0, 0, 0⟩

def ExpensiveRoute : RouteCost :=
  ⟨2, 0, 0, 0, 0, 0⟩

def ExpensiveOnly (cost : RouteCost) : Prop :=
  cost = ExpensiveRoute

def CheapOrExpensive (cost : RouteCost) : Prop :=
  cost = CheapRoute ∨ cost = ExpensiveRoute

def CanExpand (remainingExpansionFuel : Nat) : Prop :=
  0 < remainingExpansionFuel

def ConsumeExpansionFuel : Nat → Nat
  | 0 => 0
  | Nat.succ remaining => remaining

def MaximumGeneratedPaths
    (expansionFuel branchingMaximum : Nat) : Nat :=
  expansionFuel * branchingMaximum

theorem stay_path_has_zero_local_cost
    (Edge : Nat → Nat → Prop)
    (edgeCost :
      ∀ {source target},
        Edge source target → PathLocalCost)
    (node : Nat) :
    PathLocalCostOf
      edgeCost
      (EvidencePath.stay (Edge := Edge) node)
      = ZeroPathLocalCost := by
  rfl

theorem step_path_accumulates_local_cost
    {Edge : Nat → Nat → Prop}
    (edgeCost :
      ∀ {source target},
        Edge source target → PathLocalCost)
    {source next target : Nat}
    (edge : Edge source next)
    (tail : EvidencePath Edge next target) :
    PathLocalCostOf
      edgeCost
      (EvidencePath.step (Edge := Edge) edge tail)
      =
    AddPathLocalCost
      (PathLocalCostOf edgeCost tail)
      (edgeCost edge) := by
  rfl

theorem assembled_path_route_derives_graph_hops
    {Edge : Nat → Nat → Prop}
    {source target : Nat}
    (path : EvidencePath Edge source target)
    (pathLocal : PathLocalCost)
    (schedule : ExecutionScheduleCost) :
    (AssemblePathRouteCost path pathLocal schedule).graphHops
      = path.hopCount := by
  rfl

theorem local_cost_is_no_worse_before_cycle_addition
    (base cycle : PathLocalCost) :
    LocalNoWorse base (AddPathLocalCost base cycle) := by
  exact ⟨
    Nat.le_add_right
      base.encodedReceiptBytes
      cycle.encodedReceiptBytes,
    Nat.le_add_right
      base.verificationOps
      cycle.verificationOps⟩

theorem full_route_cycle_removal_requires_schedule_monotonicity
    (baseHops cycleHops : Nat)
    (baseLocal cycleLocal : PathLocalCost)
    (withoutCycle withCycle : ExecutionScheduleCost)
    (scheduleBound : ExecutionNoWorse withoutCycle withCycle) :
    NoWorse
      (AssembleRouteCostFromParts
        baseHops
        baseLocal
        withoutCycle)
      (AssembleRouteCostFromParts
        (baseHops + cycleHops)
        (AddPathLocalCost baseLocal cycleLocal)
        withCycle) := by
  exact ⟨
    Nat.le_add_right baseHops cycleHops,
    scheduleBound.1,
    scheduleBound.2.1,
    Nat.le_add_right baseLocal.verificationOps cycleLocal.verificationOps,
    scheduleBound.2.2.1,
    scheduleBound.2.2.2⟩

theorem positive_hop_cycle_removal_strictly_dominates
    (baseHops cycleHops : Nat)
    (baseLocal cycleLocal : PathLocalCost)
    (withoutCycle withCycle : ExecutionScheduleCost)
    (positiveCycle : 0 < cycleHops)
    (scheduleBound : ExecutionNoWorse withoutCycle withCycle) :
    StrictlyDominates
      (AssembleRouteCostFromParts
        baseHops
        baseLocal
        withoutCycle)
      (AssembleRouteCostFromParts
        (baseHops + cycleHops)
        (AddPathLocalCost baseLocal cycleLocal)
        withCycle) := by
  constructor
  · exact full_route_cycle_removal_requires_schedule_monotonicity
      baseHops
      cycleHops
      baseLocal
      cycleLocal
      withoutCycle
      withCycle
      scheduleBound
  · apply Or.inl
    have strict :=
      Nat.add_lt_add_left positiveCycle baseHops
    rw [Nat.add_zero] at strict
    exact strict

theorem path_local_improvement_alone_does_not_imply_full_route_order :
    let shorterButMoreRounds : RouteCost :=
      ⟨1, 10, 0, 0, 0, 0⟩
    let longerButFewerRounds : RouteCost :=
      ⟨2, 0, 0, 0, 0, 0⟩
    ParetoIncomparable
      shorterButMoreRounds
      longerButFewerRounds := by
  dsimp
  constructor
  · intro shorterNoWorse
    exact Nat.not_succ_le_zero 9 shorterNoWorse.2.1
  · intro longerNoWorse
    exact Nat.not_succ_le_self 1 longerNoWorse.1

theorem projected_token_count_has_strict_nonadditive_witness :
    SaturatingTokenCount 2
      < SaturatingTokenCount 1 + SaturatingTokenCount 1 := by
  exact Nat.lt_succ_self 1

theorem singleton_antichain_does_not_imply_pareto_coverage :
    FrontierAntichain ExpensiveOnly
      ∧ ¬ ParetoCovers ExpensiveOnly CheapOrExpensive := by
  constructor
  · intro left right leftMember rightMember different
    unfold ExpensiveOnly at leftMember rightMember
    exact (different (leftMember.trans rightMember.symm)).elim
  · intro covers
    obtain ⟨frontierCost, frontierMember, bound⟩ :=
      covers CheapRoute (Or.inl rfl)
    unfold ExpensiveOnly at frontierMember
    rw [frontierMember] at bound
    unfold ExpensiveRoute CheapRoute at bound
    exact Nat.not_succ_le_self 1 bound.1

theorem positive_expansion_consumes_fuel
    (remaining : Nat) :
    ConsumeExpansionFuel (Nat.succ remaining)
      < Nat.succ remaining := by
  exact Nat.lt_succ_self remaining

theorem zero_expansion_fuel_blocks_expansion :
    ¬ CanExpand 0 := by
  exact Nat.lt_irrefl 0

theorem generated_path_bound_is_monotone_in_fuel
    (usedFuel totalFuel branchingMaximum : Nat)
    (fuelBound : usedFuel ≤ totalFuel) :
    MaximumGeneratedPaths usedFuel branchingMaximum
      ≤ MaximumGeneratedPaths totalFuel branchingMaximum := by
  exact Nat.mul_le_mul_right branchingMaximum fuelBound

end ASPProof.SearchRouteMultiobjectivePathCostFrontierTermination
