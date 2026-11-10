-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import Lean.Elab.Tactic.Omega

namespace ASPProof.SearchRouteCompletionEquivalentPareto

structure RouteCost where
  graphHops : Nat
  toolRounds : Nat
  searchTokens : Nat
  uncachedModelTokens : Nat
  deriving DecidableEq, Repr

structure RouteBudget where
  maxGraphHops : Nat
  maxToolRounds : Nat
  maxSearchTokens : Nat
  maxUncachedModelTokens : Nat
  deriving DecidableEq, Repr

structure CompletionKey where
  generation : Nat
  coverageDigest : Nat
  modeDigest : Nat
  deriving DecidableEq, Repr

structure RoutePlan where
  initialPotential : Nat
  finalPotential : Nat
  completionKey : CompletionKey
  cost : RouteCost
  deriving DecidableEq, Repr

def CostNoWorse (left right : RouteCost) : Prop :=
  left.graphHops ≤ right.graphHops
    ∧ left.toolRounds ≤ right.toolRounds
    ∧ left.searchTokens ≤ right.searchTokens
    ∧ left.uncachedModelTokens ≤ right.uncachedModelTokens

def StrictCostAxis (left right : RouteCost) : Prop :=
  left.graphHops < right.graphHops
    ∨ left.toolRounds < right.toolRounds
    ∨ left.searchTokens < right.searchTokens
    ∨ left.uncachedModelTokens < right.uncachedModelTokens

def CompletionEquivalent (left right : RoutePlan) : Prop :=
  left.initialPotential = right.initialPotential
    ∧ left.finalPotential = right.finalPotential
    ∧ left.completionKey = right.completionKey

def Dominates (left right : RoutePlan) : Prop :=
  CompletionEquivalent left right
    ∧ CostNoWorse left.cost right.cost
    ∧ StrictCostAxis left.cost right.cost

def WithinBudget (cost : RouteCost) (budget : RouteBudget) : Prop :=
  cost.graphHops ≤ budget.maxGraphHops
    ∧ cost.toolRounds ≤ budget.maxToolRounds
    ∧ cost.searchTokens ≤ budget.maxSearchTokens
    ∧ cost.uncachedModelTokens ≤ budget.maxUncachedModelTokens

def ParetoMinimal
    (admissible : RoutePlan → Prop)
    (chosen : RoutePlan) : Prop :=
  admissible chosen
    ∧ ∀ alternative,
        admissible alternative →
        ¬ Dominates alternative chosen

structure ActionEstimate where
  potentialDecrease : Nat
  searchTokens : Nat
  deriving DecidableEq, Repr

def LocallyMoreEfficient
    (left right : ActionEstimate) : Prop :=
  left.potentialDecrease * right.searchTokens
    > right.potentialDecrease * left.searchTokens

theorem cost_no_worse_reflexive
    (cost : RouteCost) :
    CostNoWorse cost cost := by
  simp [CostNoWorse]

theorem cost_no_worse_transitive
    (first second third : RouteCost)
    (firstSecond : CostNoWorse first second)
    (secondThird : CostNoWorse second third) :
    CostNoWorse first third := by
  rcases firstSecond with
    ⟨firstHops, firstRounds, firstSearch, firstModel⟩
  rcases secondThird with
    ⟨secondHops, secondRounds, secondSearch, secondModel⟩
  exact
    ⟨ Nat.le_trans firstHops secondHops
    , Nat.le_trans firstRounds secondRounds
    , Nat.le_trans firstSearch secondSearch
    , Nat.le_trans firstModel secondModel
    ⟩

theorem completion_equivalent_transitive
    (first second third : RoutePlan)
    (firstSecond : CompletionEquivalent first second)
    (secondThird : CompletionEquivalent second third) :
    CompletionEquivalent first third := by
  rcases firstSecond with
    ⟨firstInitial, firstFinal, firstCompletion⟩
  rcases secondThird with
    ⟨secondInitial, secondFinal, secondCompletion⟩
  exact
    ⟨ firstInitial.trans secondInitial
    , firstFinal.trans secondFinal
    , firstCompletion.trans secondCompletion
    ⟩

theorem dominance_is_irreflexive
    (route : RoutePlan) :
    ¬ Dominates route route := by
  intro dominates
  rcases dominates.2.2 with hops | rounds | search | model <;> omega

theorem dominance_is_transitive
    (first second third : RoutePlan)
    (firstSecond : Dominates first second)
    (secondThird : Dominates second third) :
    Dominates first third := by
  rcases firstSecond with
    ⟨firstCompletion, firstCost, firstStrict⟩
  rcases secondThird with
    ⟨secondCompletion, secondCost, _secondStrict⟩
  refine
    ⟨ completion_equivalent_transitive
        first second third firstCompletion secondCompletion
    , cost_no_worse_transitive
        first.cost second.cost third.cost firstCost secondCost
    , ?_
    ⟩
  rcases firstStrict with hops | rounds | search | model
  · exact Or.inl (Nat.lt_of_lt_of_le hops secondCost.1)
  · exact Or.inr
      (Or.inl (Nat.lt_of_lt_of_le rounds secondCost.2.1))
  · exact Or.inr
      (Or.inr
        (Or.inl
          (Nat.lt_of_lt_of_le search secondCost.2.2.1)))
  · exact Or.inr
      (Or.inr
        (Or.inr
          (Nat.lt_of_lt_of_le model secondCost.2.2.2)))

theorem dominance_preserves_completion
    (left right : RoutePlan)
    (dominates : Dominates left right) :
    CompletionEquivalent left right :=
  dominates.1

theorem dominated_route_is_not_pareto_minimal
    (admissible : RoutePlan → Prop)
    (better dominated : RoutePlan)
    (betterAdmissible : admissible better)
    (dominates : Dominates better dominated) :
    ¬ ParetoMinimal admissible dominated := by
  intro minimal
  exact minimal.2 better betterAdmissible dominates

def exactCompletion : CompletionKey :=
  ⟨8, 42, 0⟩

def greedyRoute : RoutePlan where
  initialPotential := 3
  finalPotential := 0
  completionKey := exactCompletion
  cost := ⟨2, 2, 101, 1000⟩

def globalRoute : RoutePlan where
  initialPotential := 3
  finalPotential := 0
  completionKey := exactCompletion
  cost := ⟨3, 2, 4, 10⟩

def cheapIncompleteRoute : RoutePlan where
  initialPotential := 3
  finalPotential := 1
  completionKey := ⟨8, 7, 2⟩
  cost := ⟨1, 1, 1, 1⟩

def greedyFirst : ActionEstimate :=
  ⟨2, 1⟩

def globalFirst : ActionEstimate :=
  ⟨1, 2⟩

def exampleBudget : RouteBudget :=
  ⟨3, 2, 10, 20⟩

theorem fewer_hops_do_not_imply_full_cost_dominance :
    greedyRoute.cost.graphHops ≤ globalRoute.cost.graphHops
      ∧ ¬ CostNoWorse greedyRoute.cost globalRoute.cost := by
  simp [greedyRoute, globalRoute, CostNoWorse]

theorem local_efficiency_does_not_imply_global_dominance :
    LocallyMoreEfficient greedyFirst globalFirst
      ∧ CompletionEquivalent greedyRoute globalRoute
      ∧ ¬ CostNoWorse greedyRoute.cost globalRoute.cost := by
  simp
    [ LocallyMoreEfficient
    , greedyFirst
    , globalFirst
    , CompletionEquivalent
    , greedyRoute
    , globalRoute
    , exactCompletion
    , CostNoWorse
    ]

theorem cheaper_incomplete_route_cannot_dominate_exact :
    CostNoWorse cheapIncompleteRoute.cost globalRoute.cost
      ∧ ¬ Dominates cheapIncompleteRoute globalRoute := by
  simp
    [ CostNoWorse
    , Dominates
    , CompletionEquivalent
    , cheapIncompleteRoute
    , globalRoute
    , exactCompletion
    ]

theorem longer_complete_route_meets_budget_while_shorter_fails :
    WithinBudget globalRoute.cost exampleBudget
      ∧ ¬ WithinBudget greedyRoute.cost exampleBudget := by
  simp
    [ WithinBudget
    , globalRoute
    , greedyRoute
    , exampleBudget
    ]

end ASPProof.SearchRouteCompletionEquivalentPareto
