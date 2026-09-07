-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteGraphCostVector

namespace ASPProof.SearchRouteFeasibilityFirst

open ASPProof.SearchRouteGraphCostVector

structure RouteBudget where
  graphHops : Nat
  uncachedTokens : Nat
  rounds : Nat
  transitions : Nat
  deriving DecidableEq, Repr

def Feasible (budget : RouteBudget) (cost : GraphCostVector) : Prop :=
  cost.graphHops ≤ budget.graphHops ∧
  cost.uncachedTokens ≤ budget.uncachedTokens ∧
  cost.rounds ≤ budget.rounds ∧
  cost.transitions ≤ budget.transitions

def AdmittedNoWorse
    (budget : RouteBudget)
    (left right : GraphCostVector) : Prop :=
  Feasible budget left ∧
  (¬ Feasible budget right ∨ CostVectorNoWorse left right)

theorem admitted_no_worse_implies_feasible
    (budget : RouteBudget)
    (left right : GraphCostVector)
    (admitted : AdmittedNoWorse budget left right) :
    Feasible budget left :=
  admitted.1

def exampleBudget : RouteBudget :=
  {
    graphHops := 4
    uncachedTokens := 20
    rounds := 4
    transitions := 4
  }

def shortExpensive : GraphCostVector :=
  {
    graphHops := 1
    uncachedTokens := 100
    rounds := 1
    transitions := 1
  }

def longerExecutable : GraphCostVector :=
  {
    graphHops := 2
    uncachedTokens := 10
    rounds := 1
    transitions := 1
  }

theorem hop_first_prefers_short_expensive :
    CostVectorNoWorse shortExpensive longerExecutable := by
  unfold CostVectorNoWorse TokenTailNoWorse RoundTransitionNoWorse
    shortExpensive longerExecutable
  decide

theorem short_expensive_is_infeasible :
    ¬ Feasible exampleBudget shortExpensive := by
  unfold Feasible exampleBudget shortExpensive
  decide

theorem longer_route_is_feasible :
    Feasible exampleBudget longerExecutable := by
  unfold Feasible exampleBudget longerExecutable
  decide

theorem hop_first_preference_does_not_imply_feasibility :
    CostVectorNoWorse shortExpensive longerExecutable ∧
      ¬ Feasible exampleBudget shortExpensive ∧
      Feasible exampleBudget longerExecutable := by
  exact
    ⟨
      hop_first_prefers_short_expensive,
      short_expensive_is_infeasible,
      longer_route_is_feasible
    ⟩

theorem longer_route_is_admitted_over_short_route :
    AdmittedNoWorse exampleBudget longerExecutable shortExpensive := by
  exact
    ⟨
      longer_route_is_feasible,
      Or.inl short_expensive_is_infeasible
    ⟩

end ASPProof.SearchRouteFeasibilityFirst
