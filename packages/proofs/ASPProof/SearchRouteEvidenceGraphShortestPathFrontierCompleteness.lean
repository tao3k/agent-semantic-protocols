-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteGraphRouterParetoCostSelection

namespace ASPProof.SearchRouteEvidenceGraphShortestPathFrontierCompleteness

inductive EvidencePath
    (Edge : Nat → Nat → Prop) : Nat → Nat → Type
  | stay (node : Nat) : EvidencePath Edge node node
  | step
      {source next target : Nat}
      (edge : Edge source next)
      (tail : EvidencePath Edge next target) :
      EvidencePath Edge source target

def EvidencePath.hopCount
    {Edge : Nat → Nat → Prop}
    {source target : Nat} :
    EvidencePath Edge source target → Nat
  | .stay _ => 0
  | .step _ tail => Nat.succ tail.hopCount

def EvidencePath.map
    {SubEdge SuperEdge : Nat → Nat → Prop}
    (lift :
      ∀ {source target},
        SubEdge source target → SuperEdge source target)
    {source target : Nat}
    (path : EvidencePath SubEdge source target) :
    EvidencePath SuperEdge source target :=
  match path with
  | .stay node => .stay node
  | .step edge tail =>
      .step (lift edge) (EvidencePath.map lift tail)

def IsShortest
    {Edge : Nat → Nat → Prop}
    {source target : Nat}
    (candidate : EvidencePath Edge source target) : Prop :=
  ∀ alternative : EvidencePath Edge source target,
    candidate.hopCount ≤ alternative.hopCount

def HopCompleteBetween
    (FrontierEdge GlobalEdge : Nat → Nat → Prop)
    (source target : Nat) : Prop :=
  ∀ globalPath : EvidencePath GlobalEdge source target,
    ∃ frontierPath : EvidencePath FrontierEdge source target,
      frontierPath.hopCount ≤ globalPath.hopCount

def PathWithinHopBudget
    {Edge : Nat → Nat → Prop}
    {source target : Nat}
    (hopBudget : Nat)
    (path : EvidencePath Edge source target) : Prop :=
  path.hopCount ≤ hopBudget

def AuthorizedEdge
    (Edge Authorized : Nat → Nat → Prop)
    (source target : Nat) : Prop :=
  Edge source target ∧ Authorized source target

def HopClaimValid
    {Edge : Nat → Nat → Prop}
    {source target : Nat}
    (claimedHops : Nat)
    (path : EvidencePath Edge source target) : Prop :=
  claimedHops = path.hopCount

def MinimalAmong
    (candidateCost : Nat)
    (AvailableCost : Nat → Prop) : Prop :=
  AvailableCost candidateCost
    ∧ ∀ alternativeCost,
      AvailableCost alternativeCost →
        candidateCost ≤ alternativeCost

def FrontierOnlyTwo (cost : Nat) : Prop :=
  cost = 2

def GlobalOneOrTwo (cost : Nat) : Prop :=
  cost = 1 ∨ cost = 2

def AdmissibleEstimate
    (estimate actualRemainingCost : Nat) : Prop :=
  estimate ≤ actualRemainingCost

def LowerBoundsUnseen
    (lowerBound : Nat)
    (UnseenPathCost : Nat → Prop) : Prop :=
  ∀ unseenCost,
    UnseenPathCost unseenCost → lowerBound ≤ unseenCost

def GlobalUnreachableClaimAuthorized
    (frontierComplete : Bool) : Prop :=
  frontierComplete = true

theorem stay_path_has_zero_hops
    (Edge : Nat → Nat → Prop)
    (node : Nat) :
    (EvidencePath.stay (Edge := Edge) node).hopCount = 0 := by
  rfl

theorem step_path_adds_one_hop
    {Edge : Nat → Nat → Prop}
    {source next target : Nat}
    (edge : Edge source next)
    (tail : EvidencePath Edge next target) :
    (EvidencePath.step edge tail).hopCount
      = Nat.succ tail.hopCount := by
  rfl

theorem map_preserves_hop_count
    {SubEdge SuperEdge : Nat → Nat → Prop}
    (lift :
      ∀ {source target},
        SubEdge source target → SuperEdge source target)
    {source target : Nat}
    (path : EvidencePath SubEdge source target) :
    (EvidencePath.map lift path).hopCount = path.hopCount := by
  induction path with
  | stay node =>
      rfl
  | step edge tail inductionHypothesis =>
      unfold EvidencePath.map EvidencePath.hopCount
      rw [inductionHypothesis]

theorem global_shortest_implies_shortest_in_sound_frontier
    {FrontierEdge GlobalEdge : Nat → Nat → Prop}
    (sound :
      ∀ {source target},
        FrontierEdge source target → GlobalEdge source target)
    {source target : Nat}
    (candidate : EvidencePath FrontierEdge source target)
    (globallyShortest :
      IsShortest (EvidencePath.map sound candidate)) :
    IsShortest candidate := by
  intro alternative
  calc
    candidate.hopCount
        = (EvidencePath.map sound candidate).hopCount :=
      (map_preserves_hop_count sound candidate).symm
    _ ≤ (EvidencePath.map sound alternative).hopCount :=
      globallyShortest (EvidencePath.map sound alternative)
    _ = alternative.hopCount :=
      map_preserves_hop_count sound alternative

theorem shortest_in_hop_complete_frontier_implies_global_shortest
    {FrontierEdge GlobalEdge : Nat → Nat → Prop}
    (sound :
      ∀ {source target},
        FrontierEdge source target → GlobalEdge source target)
    {source target : Nat}
    (candidate : EvidencePath FrontierEdge source target)
    (frontierShortest : IsShortest candidate)
    (complete :
      HopCompleteBetween
        FrontierEdge
        GlobalEdge
        source
        target) :
    IsShortest (EvidencePath.map sound candidate) := by
  intro globalAlternative
  obtain ⟨frontierAlternative, frontierBound⟩ :=
    complete globalAlternative
  calc
    (EvidencePath.map sound candidate).hopCount
        = candidate.hopCount :=
      map_preserves_hop_count sound candidate
    _ ≤ frontierAlternative.hopCount :=
      frontierShortest frontierAlternative
    _ ≤ globalAlternative.hopCount :=
      frontierBound

theorem frontier_minimum_alone_does_not_imply_global_minimum :
    MinimalAmong 2 FrontierOnlyTwo
      ∧ ¬ MinimalAmong 2 GlobalOneOrTwo := by
  constructor
  · constructor
    · rfl
    · intro alternativeCost available
      unfold FrontierOnlyTwo at available
      rw [available]
      exact Nat.le_refl 2
  · intro globalMinimum
    have impossible :=
      globalMinimum.2 1 (Or.inl rfl)
    exact Nat.not_succ_le_self 1 impossible

theorem authorized_edge_is_a_graph_edge
    (Edge Authorized : Nat → Nat → Prop)
    {source target : Nat}
    (edge : AuthorizedEdge Edge Authorized source target) :
    Edge source target := by
  exact edge.1

theorem shortest_path_inherits_hop_budget_from_feasible_alternative
    {Edge : Nat → Nat → Prop}
    {source target : Nat}
    (candidate alternative : EvidencePath Edge source target)
    (shortest : IsShortest candidate)
    (hopBudget : Nat)
    (alternativeFeasible :
      PathWithinHopBudget hopBudget alternative) :
    PathWithinHopBudget hopBudget candidate := by
  exact Nat.le_trans
    (shortest alternative)
    alternativeFeasible

theorem correct_hop_claim_is_valid
    {Edge : Nat → Nat → Prop}
    {source target : Nat}
    (path : EvidencePath Edge source target) :
    HopClaimValid path.hopCount path := by
  rfl

theorem changed_hop_claim_is_invalid
    {Edge : Nat → Nat → Prop}
    {source target : Nat}
    (path : EvidencePath Edge source target)
    (claimedHops : Nat)
    (changed : claimedHops ≠ path.hopCount) :
    ¬ HopClaimValid claimedHops path := by
  exact changed

theorem exact_remaining_cost_is_admissible
    (remainingCost : Nat) :
    AdmissibleEstimate remainingCost remainingCost := by
  exact Nat.le_refl remainingCost

theorem overestimating_remaining_cost_is_not_admissible
    (remainingCost : Nat) :
    ¬ AdmissibleEstimate
      (Nat.succ remainingCost)
      remainingCost := by
  exact Nat.not_succ_le_self remainingCost

theorem candidate_below_unseen_lower_bound_beats_every_unseen_path
    (candidateCost lowerBound : Nat)
    (UnseenPathCost : Nat → Prop)
    (candidateBound : candidateCost ≤ lowerBound)
    (unseenBound :
      LowerBoundsUnseen lowerBound UnseenPathCost) :
    ∀ unseenCost,
      UnseenPathCost unseenCost →
        candidateCost ≤ unseenCost := by
  intro unseenCost unseen
  exact Nat.le_trans
    candidateBound
    (unseenBound unseenCost unseen)

theorem complete_frontier_authorizes_global_unreachable_claim :
    GlobalUnreachableClaimAuthorized true := by
  rfl

theorem truncated_frontier_cannot_authorize_global_unreachable_claim :
    ¬ GlobalUnreachableClaimAuthorized false := by
  intro authorized
  cases authorized

end ASPProof.SearchRouteEvidenceGraphShortestPathFrontierCompleteness
