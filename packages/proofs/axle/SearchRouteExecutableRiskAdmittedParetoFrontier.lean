import Mathlib

namespace ASPProof.AXLE.SearchRouteExecutableRiskAdmittedParetoFrontier

structure RouteCost where
  graphHops : Nat
  interactionRounds : Nat
  searchTokens : Nat
  modelTokens : Nat
  deriving DecidableEq

structure Candidate where
  routeId : Nat
  completionKey : Nat
  routeFeasible : Bool
  riskFeasible : Bool
  cost : RouteCost
  deriving DecidableEq

def admitted (candidate : Candidate) : Prop :=
  candidate.routeFeasible = true ∧ candidate.riskFeasible = true

def completionEquivalent (left right : Candidate) : Prop :=
  left.completionKey = right.completionKey

def costNoWorse (left right : RouteCost) : Prop :=
  left.graphHops ≤ right.graphHops ∧
  left.interactionRounds ≤ right.interactionRounds ∧
  left.searchTokens ≤ right.searchTokens ∧
  left.modelTokens ≤ right.modelTokens

def strictCostImprovement (left right : RouteCost) : Prop :=
  left.graphHops < right.graphHops ∨
  left.interactionRounds < right.interactionRounds ∨
  left.searchTokens < right.searchTokens ∨
  left.modelTokens < right.modelTokens

def dominates (left right : Candidate) : Prop :=
  admitted left ∧
  admitted right ∧
  completionEquivalent left right ∧
  costNoWorse left.cost right.cost ∧
  strictCostImprovement left.cost right.cost

def unsafeShort : Candidate :=
  { routeId := 1
    completionKey := 100
    routeFeasible := true
    riskFeasible := false
    cost := { graphHops := 1, interactionRounds := 1, searchTokens := 60, modelTokens := 10 } }

def cachedUnsafeShort : Candidate :=
  { routeId := 2
    completionKey := 100
    routeFeasible := true
    riskFeasible := false
    cost := { graphHops := 1, interactionRounds := 1, searchTokens := 40, modelTokens := 0 } }

def safeLong : Candidate :=
  { routeId := 3
    completionKey := 100
    routeFeasible := true
    riskFeasible := true
    cost := { graphHops := 2, interactionRounds := 2, searchTokens := 80, modelTokens := 20 } }

def dominatedSafe : Candidate :=
  { routeId := 4
    completionKey := 100
    routeFeasible := true
    riskFeasible := true
    cost := { graphHops := 3, interactionRounds := 3, searchTokens := 90, modelTokens := 30 } }

def equalCostDistinct : Candidate :=
  { routeId := 5
    completionKey := 100
    routeFeasible := true
    riskFeasible := true
    cost := { graphHops := 2, interactionRounds := 2, searchTokens := 80, modelTokens := 20 } }

def differentCompletion : Candidate :=
  { routeId := 6
    completionKey := 101
    routeFeasible := true
    riskFeasible := true
    cost := { graphHops := 1, interactionRounds := 1, searchTokens := 10, modelTokens := 5 } }

theorem unsafe_shorter_route_does_not_dominate_admitted_route :
    ¬ dominates unsafeShort safeLong := by
  norm_num [dominates, admitted, unsafeShort, safeLong]

theorem cache_improvement_does_not_create_admission_or_dominance :
    ¬ admitted cachedUnsafeShort ∧ ¬ dominates cachedUnsafeShort safeLong := by
  norm_num [dominates, admitted, cachedUnsafeShort, safeLong]

theorem admitted_dominated_route_has_strict_witness :
    dominates safeLong dominatedSafe := by
  norm_num [dominates, admitted, completionEquivalent, costNoWorse,
    strictCostImprovement, safeLong, dominatedSafe]

theorem different_completion_classes_are_incomparable :
    ¬ dominates differentCompletion safeLong := by
  norm_num [dominates, admitted, completionEquivalent, costNoWorse,
    strictCostImprovement, differentCompletion, safeLong]

theorem equal_cost_distinct_routes_do_not_dominate :
    ¬ dominates safeLong equalCostDistinct ∧
    ¬ dominates equalCostDistinct safeLong := by
  norm_num [dominates, admitted, completionEquivalent, costNoWorse,
    strictCostImprovement, safeLong, equalCostDistinct]

end ASPProof.AXLE.SearchRouteExecutableRiskAdmittedParetoFrontier
