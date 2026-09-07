-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteCanonicalReceiptGrammarProgressiveInspectBound

namespace ASPProof.SearchRouteGraphRouterParetoCostSelection

structure RouteCost where
  /-- Certificate-bound semantic evidence-path length, never cache-adjusted. -/
  graphHops : Nat
  interactionRounds : Nat
  projectedTokens : Nat
  verificationOps : Nat
  searchExecutions : Nat
  modelPrefixRecomputations : Nat

def NoWorse
    (left right : RouteCost) : Prop :=
  left.graphHops ≤ right.graphHops
    ∧ left.interactionRounds ≤ right.interactionRounds
    ∧ left.projectedTokens ≤ right.projectedTokens
    ∧ left.verificationOps ≤ right.verificationOps
    ∧ left.searchExecutions ≤ right.searchExecutions
    ∧ left.modelPrefixRecomputations
      ≤ right.modelPrefixRecomputations

def StrictlyDominates
    (left right : RouteCost) : Prop :=
  NoWorse left right
    ∧ (
      left.graphHops < right.graphHops
      ∨ left.interactionRounds < right.interactionRounds
      ∨ left.projectedTokens < right.projectedTokens
      ∨ left.verificationOps < right.verificationOps
      ∨ left.searchExecutions < right.searchExecutions
      ∨ left.modelPrefixRecomputations
        < right.modelPrefixRecomputations)

def ParetoIncomparable
    (left right : RouteCost) : Prop :=
  ¬ NoWorse left right
    ∧ ¬ NoWorse right left

def Feasible
    (cost caps : RouteCost) : Prop :=
  NoWorse cost caps

def HopFirstLexBetter
    (left right : RouteCost) : Prop :=
  left.graphHops < right.graphHops
    ∨ (left.graphHops = right.graphHops
      ∧ (left.interactionRounds < right.interactionRounds
        ∨ (left.interactionRounds = right.interactionRounds
          ∧ (left.projectedTokens < right.projectedTokens
            ∨ (left.projectedTokens = right.projectedTokens
              ∧ (left.verificationOps < right.verificationOps
                ∨ (left.verificationOps = right.verificationOps
                  ∧ (left.searchExecutions < right.searchExecutions
                    ∨ (left.searchExecutions = right.searchExecutions
                      ∧ left.modelPrefixRecomputations
                        < right.modelPrefixRecomputations)))))))))

structure RouteWeights where
  graphHops : Nat
  interactionRounds : Nat
  projectedTokens : Nat
  verificationOps : Nat
  searchExecutions : Nat
  modelPrefixRecomputations : Nat

def WeightedScore
    (weights : RouteWeights)
    (cost : RouteCost) : Nat :=
  weights.graphHops * cost.graphHops
    + weights.interactionRounds * cost.interactionRounds
    + weights.projectedTokens * cost.projectedTokens
    + weights.verificationOps * cost.verificationOps
    + weights.searchExecutions * cost.searchExecutions
    + weights.modelPrefixRecomputations
      * cost.modelPrefixRecomputations

def UnitWeights : RouteWeights :=
  ⟨1, 1, 1, 1, 1, 1⟩

def CombinedCacheWork
    (cost : RouteCost) : Nat :=
  cost.searchExecutions + cost.modelPrefixRecomputations

structure RouteEvaluationIdentity where
  workspaceSnapshotDigest : Nat
  queryDigest : Nat
  searchPolicyDigest : Nat
  modelDigest : Nat
  promptAndToolDigest : Nat
  budgetVectorDigest : Nat

def RouteEvaluationComparable
    (left right : RouteEvaluationIdentity) : Prop :=
  left = right

theorem no_worse_is_reflexive
    (cost : RouteCost) :
    NoWorse cost cost := by
  exact ⟨
    Nat.le_refl cost.graphHops,
    Nat.le_refl cost.interactionRounds,
    Nat.le_refl cost.projectedTokens,
    Nat.le_refl cost.verificationOps,
    Nat.le_refl cost.searchExecutions,
    Nat.le_refl cost.modelPrefixRecomputations⟩

theorem no_worse_is_transitive
    (first second third : RouteCost)
    (firstSecond : NoWorse first second)
    (secondThird : NoWorse second third) :
    NoWorse first third := by
  exact ⟨
    Nat.le_trans firstSecond.1 secondThird.1,
    Nat.le_trans firstSecond.2.1 secondThird.2.1,
    Nat.le_trans firstSecond.2.2.1 secondThird.2.2.1,
    Nat.le_trans firstSecond.2.2.2.1 secondThird.2.2.2.1,
    Nat.le_trans firstSecond.2.2.2.2.1 secondThird.2.2.2.2.1,
    Nat.le_trans firstSecond.2.2.2.2.2 secondThird.2.2.2.2.2⟩

theorem strict_dominance_implies_no_worse
    (left right : RouteCost)
    (dominates : StrictlyDominates left right) :
    NoWorse left right := by
  exact dominates.1

theorem strict_dominance_is_irreflexive
    (cost : RouteCost) :
    ¬ StrictlyDominates cost cost := by
  intro dominates
  rcases dominates.2 with
    graphHopsBetter
    | interactionRoundsBetter
    | projectedTokensBetter
    | verificationOpsBetter
    | searchExecutionsBetter
    | modelPrefixRecomputationsBetter
  · exact Nat.lt_irrefl cost.graphHops graphHopsBetter
  · exact Nat.lt_irrefl cost.interactionRounds interactionRoundsBetter
  · exact Nat.lt_irrefl cost.projectedTokens projectedTokensBetter
  · exact Nat.lt_irrefl cost.verificationOps verificationOpsBetter
  · exact Nat.lt_irrefl cost.searchExecutions searchExecutionsBetter
  · exact Nat.lt_irrefl
      cost.modelPrefixRecomputations
      modelPrefixRecomputationsBetter

theorem no_worse_route_preserves_feasibility
    (better worse caps : RouteCost)
    (betterBound : NoWorse better worse)
    (worseFeasible : Feasible worse caps) :
    Feasible better caps := by
  exact no_worse_is_transitive
    better
    worse
    caps
    betterBound
    worseFeasible

theorem fewer_graph_hops_is_hop_first_lex_better
    (left right : RouteCost)
    (fewerHops : left.graphHops < right.graphHops) :
    HopFirstLexBetter left right := by
  exact Or.inl fewerHops

theorem hop_first_lex_order_requires_prior_feasibility
    (tokenCap : Nat) :
    let unsafeRoute : RouteCost :=
      ⟨0, 0, Nat.succ tokenCap, 0, 0, 0⟩
    let alternativeRoute : RouteCost :=
      ⟨1, 0, 0, 0, 0, 0⟩
    let caps : RouteCost :=
      ⟨1, 0, tokenCap, 0, 0, 0⟩
    HopFirstLexBetter unsafeRoute alternativeRoute
      ∧ ¬ Feasible unsafeRoute caps := by
  dsimp
  constructor
  · exact Or.inl (Nat.zero_lt_succ 0)
  · intro feasible
    exact Nat.not_succ_le_self tokenCap feasible.2.2.1

theorem equal_unit_weight_score_can_hide_token_tradeoff
    (regression : Nat) :
    let tokenHeavy : RouteCost :=
      ⟨0, 0, Nat.succ regression, 0, 0, 0⟩
    let hopHeavy : RouteCost :=
      ⟨Nat.succ regression, 0, 0, 0, 0, 0⟩
    WeightedScore UnitWeights tokenHeavy
      = WeightedScore UnitWeights hopHeavy := by
  dsimp
  unfold WeightedScore UnitWeights
  repeat rw [Nat.one_mul]
  repeat rw [Nat.zero_add]
  repeat rw [Nat.add_zero]

theorem equal_unit_weight_routes_can_be_pareto_incomparable
    (regression : Nat) :
    let tokenHeavy : RouteCost :=
      ⟨0, 0, Nat.succ regression, 0, 0, 0⟩
    let hopHeavy : RouteCost :=
      ⟨Nat.succ regression, 0, 0, 0, 0, 0⟩
    ParetoIncomparable tokenHeavy hopHeavy := by
  dsimp
  constructor
  · intro tokenNoWorse
    exact Nat.not_succ_le_zero regression tokenNoWorse.2.2.1
  · intro hopNoWorse
    exact Nat.not_succ_le_zero regression hopNoWorse.1

theorem combined_cache_work_can_hide_cache_direction :
    let searchReused : RouteCost :=
      ⟨0, 0, 0, 0, 0, 1⟩
    let modelPrefixReused : RouteCost :=
      ⟨0, 0, 0, 0, 1, 0⟩
    CombinedCacheWork searchReused
      = CombinedCacheWork modelPrefixReused := by
  rfl

theorem equal_combined_cache_work_can_be_pareto_incomparable :
    let searchReused : RouteCost :=
      ⟨0, 0, 0, 0, 0, 1⟩
    let modelPrefixReused : RouteCost :=
      ⟨0, 0, 0, 0, 1, 0⟩
    ParetoIncomparable searchReused modelPrefixReused := by
  dsimp
  constructor
  · intro searchNoWorse
    exact Nat.not_succ_le_zero 0 searchNoWorse.2.2.2.2.2
  · intro modelNoWorse
    exact Nat.not_succ_le_zero 0 modelNoWorse.2.2.2.2.1

theorem search_reuse_does_not_imply_model_prefix_reuse :
    ∃ cost : RouteCost,
      cost.searchExecutions = 0
        ∧ cost.modelPrefixRecomputations = 1 := by
  exact ⟨⟨0, 0, 0, 0, 0, 1⟩, rfl, rfl⟩

theorem model_prefix_reuse_does_not_imply_search_reuse :
    ∃ cost : RouteCost,
      cost.modelPrefixRecomputations = 0
        ∧ cost.searchExecutions = 1 := by
  exact ⟨⟨0, 0, 0, 0, 1, 0⟩, rfl, rfl⟩

theorem unchanged_route_evaluation_identity_is_comparable
    (identity : RouteEvaluationIdentity) :
    RouteEvaluationComparable identity identity := by
  rfl

theorem changed_evaluation_snapshot_invalidates_comparison
    (workspaceSnapshotDigest changedWorkspaceSnapshotDigest
      queryDigest searchPolicyDigest modelDigest
      promptAndToolDigest budgetVectorDigest : Nat)
    (changed :
      workspaceSnapshotDigest ≠ changedWorkspaceSnapshotDigest) :
    ¬ RouteEvaluationComparable
      ⟨workspaceSnapshotDigest, queryDigest, searchPolicyDigest,
        modelDigest, promptAndToolDigest, budgetVectorDigest⟩
      ⟨changedWorkspaceSnapshotDigest, queryDigest, searchPolicyDigest,
        modelDigest, promptAndToolDigest, budgetVectorDigest⟩ := by
  intro comparable
  unfold RouteEvaluationComparable at comparable
  exact changed
    (congrArg
      RouteEvaluationIdentity.workspaceSnapshotDigest
      comparable)

theorem changed_evaluation_model_invalidates_comparison
    (workspaceSnapshotDigest queryDigest searchPolicyDigest
      modelDigest changedModelDigest
      promptAndToolDigest budgetVectorDigest : Nat)
    (changed : modelDigest ≠ changedModelDigest) :
    ¬ RouteEvaluationComparable
      ⟨workspaceSnapshotDigest, queryDigest, searchPolicyDigest,
        modelDigest, promptAndToolDigest, budgetVectorDigest⟩
      ⟨workspaceSnapshotDigest, queryDigest, searchPolicyDigest,
        changedModelDigest, promptAndToolDigest, budgetVectorDigest⟩ := by
  intro comparable
  unfold RouteEvaluationComparable at comparable
  exact changed
    (congrArg RouteEvaluationIdentity.modelDigest comparable)

end ASPProof.SearchRouteGraphRouterParetoCostSelection
