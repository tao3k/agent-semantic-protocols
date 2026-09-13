-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteBoundedAdaptiveInspectConvergence

namespace ASPProof.SearchRouteForecastBoundParetoBatchScheduling

open ASPProof.SearchRouteCertifiedRegistryEpochTransition

structure CacheForecast where
  forecastDigest : Digest
  searchHitBasisPoints : Nat
  proofHitBasisPoints : Nat
  modelPrefixHitBasisPoints : Nat
deriving DecidableEq, Repr

def ValidForecast (forecast : CacheForecast) : Prop :=
  forecast.searchHitBasisPoints ≤ 10000 ∧
    forecast.proofHitBasisPoints ≤ 10000 ∧
    forecast.modelPrefixHitBasisPoints ≤ 10000

structure GrossBatchCost where
  disclosedTokenUnits : Nat
  latencyUnits : Nat
  llmRounds : Nat
  providerCalls : Nat
  proofChecks : Nat
  uncachedModelTokens : Nat
deriving DecidableEq, Repr

structure EstimatedBatchCost where
  disclosedTokenUnits : Nat
  latencyUnits : Nat
  llmRounds : Nat
  providerWorkScaled : Nat
  proofWorkScaled : Nat
  modelTokenWorkScaled : Nat
deriving DecidableEq, Repr

structure BatchUtility where
  decisionValue : Nat
  progressEdges : Nat
deriving DecidableEq, Repr

structure BatchPlan where
  requestDigest : Digest
  chainDigest : Digest
  policyDigest : Digest
  cacheForecast : CacheForecast
  edges : List Nat
  grossCost : GrossBatchCost
  utility : BatchUtility
deriving DecidableEq, Repr

structure BatchRealization where
  forecastDigest : Digest
  realizedSearchHit : Bool
  realizedProofHit : Bool
  realizedModelPrefixHit : Bool
  realizedCost : GrossBatchCost
deriving DecidableEq, Repr

def estimatedCost (plan : BatchPlan) : EstimatedBatchCost :=
  {
    disclosedTokenUnits := plan.grossCost.disclosedTokenUnits
    latencyUnits := plan.grossCost.latencyUnits
    llmRounds := plan.grossCost.llmRounds
    providerWorkScaled :=
      plan.grossCost.providerCalls *
        (10000 - plan.cacheForecast.searchHitBasisPoints)
    proofWorkScaled :=
      plan.grossCost.proofChecks *
        (10000 - plan.cacheForecast.proofHitBasisPoints)
    modelTokenWorkScaled :=
      plan.grossCost.uncachedModelTokens *
        (10000 - plan.cacheForecast.modelPrefixHitBasisPoints)
  }

def ComparablePlans (left right : BatchPlan) : Prop :=
  left.requestDigest = right.requestDigest ∧
    left.chainDigest = right.chainDigest ∧
    left.policyDigest = right.policyDigest ∧
    left.cacheForecast = right.cacheForecast

def CostNoWorse (left right : BatchPlan) : Prop :=
  (estimatedCost left).disclosedTokenUnits ≤
      (estimatedCost right).disclosedTokenUnits ∧
    (estimatedCost left).latencyUnits ≤
      (estimatedCost right).latencyUnits ∧
    (estimatedCost left).llmRounds ≤
      (estimatedCost right).llmRounds ∧
    (estimatedCost left).providerWorkScaled ≤
      (estimatedCost right).providerWorkScaled ∧
    (estimatedCost left).proofWorkScaled ≤
      (estimatedCost right).proofWorkScaled ∧
    (estimatedCost left).modelTokenWorkScaled ≤
      (estimatedCost right).modelTokenWorkScaled

def UtilityNoWorse (left right : BatchPlan) : Prop :=
  right.utility.decisionValue ≤ left.utility.decisionValue ∧
    right.utility.progressEdges ≤ left.utility.progressEdges

def StrictlyImproves (left right : BatchPlan) : Prop :=
  (estimatedCost left).disclosedTokenUnits <
      (estimatedCost right).disclosedTokenUnits ∨
    (estimatedCost left).latencyUnits <
      (estimatedCost right).latencyUnits ∨
    (estimatedCost left).llmRounds <
      (estimatedCost right).llmRounds ∨
    (estimatedCost left).providerWorkScaled <
      (estimatedCost right).providerWorkScaled ∨
    (estimatedCost left).proofWorkScaled <
      (estimatedCost right).proofWorkScaled ∨
    (estimatedCost left).modelTokenWorkScaled <
      (estimatedCost right).modelTokenWorkScaled ∨
    right.utility.decisionValue < left.utility.decisionValue ∨
    right.utility.progressEdges < left.utility.progressEdges

def Dominates (left right : BatchPlan) : Prop :=
  ComparablePlans left right ∧
    CostNoWorse left right ∧
    UtilityNoWorse left right ∧
    StrictlyImproves left right

def FeasibleBatch (remainingEdges : List Nat) (plan : BatchPlan) : Prop :=
  ValidForecast plan.cacheForecast ∧
    plan.edges ≠ [] ∧
    plan.edges.Sublist remainingEdges ∧
    plan.utility.progressEdges = plan.edges.length ∧
    plan.edges.length ≤ plan.grossCost.disclosedTokenUnits

def AuthorizedBatch
    (chainVerified disclosureValid : Prop)
    (remainingEdges : List Nat)
    (plan : BatchPlan) : Prop :=
  chainVerified ∧ disclosureValid ∧ FeasibleBatch remainingEdges plan

def RealizationBound
    (plan : BatchPlan)
    (realization : BatchRealization) : Prop :=
  realization.forecastDigest = plan.cacheForecast.forecastDigest

def ParetoOptimal
    (remainingEdges : List Nat)
    (candidates : List BatchPlan)
    (chosen : BatchPlan) : Prop :=
  chosen ∈ candidates ∧
    FeasibleBatch remainingEdges chosen ∧
    ∀ alternative,
      alternative ∈ candidates →
      FeasibleBatch remainingEdges alternative →
      ¬Dominates alternative chosen

def stableForecast : CacheForecast :=
  {
    forecastDigest := 900
    searchHitBasisPoints := 5000
    proofHitBasisPoints := 5000
    modelPrefixHitBasisPoints := 5000
  }

def lowerTokenLowerValuePlan : BatchPlan :=
  {
    requestDigest := 1
    chainDigest := 2
    policyDigest := 3
    cacheForecast := stableForecast
    edges := [1]
    grossCost := {
      disclosedTokenUnits := 1
      latencyUnits := 2
      llmRounds := 1
      providerCalls := 1
      proofChecks := 1
      uncachedModelTokens := 100
    }
    utility := { decisionValue := 2, progressEdges := 1 }
  }

def higherTokenHigherValuePlan : BatchPlan :=
  {
    requestDigest := 1
    chainDigest := 2
    policyDigest := 3
    cacheForecast := stableForecast
    edges := [1, 3]
    grossCost := {
      disclosedTokenUnits := 2
      latencyUnits := 2
      llmRounds := 1
      providerCalls := 1
      proofChecks := 1
      uncachedModelTokens := 100
    }
    utility := { decisionValue := 8, progressEdges := 2 }
  }

theorem valid_forecast_bounds_every_probability
    {forecast : CacheForecast}
    (valid : ValidForecast forecast) :
    forecast.searchHitBasisPoints ≤ 10000 ∧
      forecast.proofHitBasisPoints ≤ 10000 ∧
      forecast.modelPrefixHitBasisPoints ≤ 10000 :=
  valid

theorem estimated_cache_work_is_lane_scoped
    (plan : BatchPlan) :
    (estimatedCost plan).providerWorkScaled =
        plan.grossCost.providerCalls *
          (10000 - plan.cacheForecast.searchHitBasisPoints) ∧
      (estimatedCost plan).proofWorkScaled =
        plan.grossCost.proofChecks *
          (10000 - plan.cacheForecast.proofHitBasisPoints) ∧
      (estimatedCost plan).modelTokenWorkScaled =
        plan.grossCost.uncachedModelTokens *
          (10000 - plan.cacheForecast.modelPrefixHitBasisPoints) :=
  ⟨rfl, rfl, rfl⟩

theorem feasible_batch_is_nonempty
    {remainingEdges : List Nat}
    {plan : BatchPlan}
    (feasible : FeasibleBatch remainingEdges plan) :
    plan.edges ≠ [] :=
  feasible.2.1

theorem feasible_batch_is_ordered_sublist
    {remainingEdges : List Nat}
    {plan : BatchPlan}
    (feasible : FeasibleBatch remainingEdges plan) :
    plan.edges.Sublist remainingEdges :=
  feasible.2.2.1

theorem feasible_batch_makes_strict_progress
    {remainingEdges : List Nat}
    {plan : BatchPlan}
    (feasible : FeasibleBatch remainingEdges plan) :
    0 < plan.utility.progressEdges := by
  rw [feasible.2.2.2.1]
  cases edgesEquation : plan.edges with
  | nil =>
      exact False.elim (feasible.2.1 edgesEquation)
  | cons head tail =>
      simp

theorem forecast_mismatch_rejects_dominance
    {left right : BatchPlan}
    (mismatch : left.cacheForecast ≠ right.cacheForecast) :
    ¬Dominates left right := by
  intro dominates
  exact mismatch dominates.1.2.2.2

theorem dominance_is_irreflexive
    (plan : BatchPlan) :
    ¬Dominates plan plan := by
  intro dominates
  obtain ⟨_, _, _, strict⟩ := dominates
  unfold StrictlyImproves at strict
  omega

theorem pareto_optimal_plan_is_not_dominated
    {remainingEdges : List Nat}
    {candidates : List BatchPlan}
    {chosen alternative : BatchPlan}
    (optimal : ParetoOptimal remainingEdges candidates chosen)
    (member : alternative ∈ candidates)
    (feasible : FeasibleBatch remainingEdges alternative) :
    ¬Dominates alternative chosen :=
  optimal.2.2 alternative member feasible

theorem dominated_candidate_is_not_pareto_optimal
    {remainingEdges : List Nat}
    {candidates : List BatchPlan}
    {chosen alternative : BatchPlan}
    (member : alternative ∈ candidates)
    (feasible : FeasibleBatch remainingEdges alternative)
    (dominates : Dominates alternative chosen) :
    ¬ParetoOptimal remainingEdges candidates chosen := by
  intro optimal
  exact (optimal.2.2 alternative member feasible) dominates

theorem lower_token_cost_alone_does_not_establish_dominance :
    (estimatedCost lowerTokenLowerValuePlan).disclosedTokenUnits <
      (estimatedCost higherTokenHigherValuePlan).disclosedTokenUnits ∧
    ¬Dominates lowerTokenLowerValuePlan higherTokenHigherValuePlan := by
  constructor
  · change 1 < 2
    decide
  · intro dominates
    have utilityNoWorse := dominates.2.2.1
    unfold UtilityNoWorse at utilityNoWorse
    dsimp [lowerTokenLowerValuePlan, higherTokenHigherValuePlan] at utilityNoWorse
    omega

theorem token_value_tradeoff_is_pareto_incomparable :
    ¬Dominates lowerTokenLowerValuePlan higherTokenHigherValuePlan ∧
    ¬Dominates higherTokenHigherValuePlan lowerTokenLowerValuePlan := by
  constructor
  · exact lower_token_cost_alone_does_not_establish_dominance.2
  · intro dominates
    have costNoWorse := dominates.2.1
    unfold CostNoWorse at costNoWorse
    dsimp [estimatedCost, lowerTokenLowerValuePlan,
      higherTokenHigherValuePlan, stableForecast] at costNoWorse
    omega

theorem equal_forecast_digest_does_not_establish_equal_forecast :
    let left : CacheForecast :=
      { forecastDigest := 44, searchHitBasisPoints := 1000,
        proofHitBasisPoints := 2000, modelPrefixHitBasisPoints := 3000 }
    let right : CacheForecast :=
      { forecastDigest := 44, searchHitBasisPoints := 9000,
        proofHitBasisPoints := 2000, modelPrefixHitBasisPoints := 3000 }
    left.forecastDigest = right.forecastDigest ∧ left ≠ right := by
  decide

theorem realization_requires_forecast_digest_binding
    {plan : BatchPlan}
    {realization : BatchRealization}
    (bound : RealizationBound plan realization) :
    realization.forecastDigest =
      plan.cacheForecast.forecastDigest :=
  bound

theorem one_forecast_allows_different_realized_cache_hits :
    let left : BatchRealization := {
      forecastDigest := 44
      realizedSearchHit := true
      realizedProofHit := false
      realizedModelPrefixHit := true
      realizedCost := {
        disclosedTokenUnits := 2
        latencyUnits := 3
        llmRounds := 1
        providerCalls := 0
        proofChecks := 1
        uncachedModelTokens := 0
      }
    }
    let right : BatchRealization := {
      forecastDigest := 44
      realizedSearchHit := false
      realizedProofHit := true
      realizedModelPrefixHit := false
      realizedCost := {
        disclosedTokenUnits := 2
        latencyUnits := 4
        llmRounds := 1
        providerCalls := 1
        proofChecks := 0
        uncachedModelTokens := 100
      }
    }
    left.forecastDigest = right.forecastDigest ∧ left ≠ right := by
  decide

theorem authorized_batch_requires_verified_chain
    {chainVerified disclosureValid : Prop}
    {remainingEdges : List Nat}
    {plan : BatchPlan}
    (authorized :
      AuthorizedBatch chainVerified disclosureValid remainingEdges plan) :
    chainVerified :=
  authorized.1

theorem authorized_batch_requires_valid_disclosure
    {chainVerified disclosureValid : Prop}
    {remainingEdges : List Nat}
    {plan : BatchPlan}
    (authorized :
      AuthorizedBatch chainVerified disclosureValid remainingEdges plan) :
    disclosureValid :=
  authorized.2.1

theorem authorized_batch_requires_feasibility
    {chainVerified disclosureValid : Prop}
    {remainingEdges : List Nat}
    {plan : BatchPlan}
    (authorized :
      AuthorizedBatch chainVerified disclosureValid remainingEdges plan) :
    FeasibleBatch remainingEdges plan :=
  authorized.2.2

end ASPProof.SearchRouteForecastBoundParetoBatchScheduling
