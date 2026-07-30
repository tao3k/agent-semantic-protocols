import ASPProof.SearchRouteCacheParametricCost

namespace ASPProof.SearchRouteCachePlanningLease

open ASPProof.SearchRouteCacheParametricCost

structure PlanningContext where
  costContext : CostContext
  runtimeGeneration : Nat
  requestDigest : Nat
  now : Nat
  deriving DecidableEq, Repr

inductive PrefixCostAuthority where
  | conservative
  | leasedHit
      (prefixKey : ModelPrefixCacheKey)
      (runtimeGeneration : Nat)
      (requestDigest : Nat)
      (expiresAt : Nat)
  | postHocHit
      (prefixKey : ModelPrefixCacheKey)
      (requestDigest : Nat)
  deriving DecidableEq, Repr

def PlanningAdmissible
    (context : PlanningContext)
    (authority : PrefixCostAuthority) : Prop :=
  match authority with
  | .conservative => True
  | .leasedHit prefixKey runtimeGeneration requestDigest expiresAt =>
      context.costContext.prefixState = .verifiedHit ∧
      context.costContext.prefixKey = prefixKey ∧
      context.runtimeGeneration = runtimeGeneration ∧
      context.requestDigest = requestDigest ∧
      context.now ≤ expiresAt
  | .postHocHit _ _ => False

def AccountingAdmissible
    (context : PlanningContext)
    (requestDigest : Nat)
    (authority : PrefixCostAuthority) : Prop :=
  match authority with
  | .postHocHit prefixKey observedRequestDigest =>
      context.costContext.prefixKey = prefixKey ∧
      requestDigest = observedRequestDigest
  | _ => False

def planningTokens
    (authority : PrefixCostAuthority)
    (witness : PromptCostWitness) : Nat :=
  match authority with
  | .leasedHit _ _ _ _ => effectiveTokens .verifiedHit witness
  | _ => conservativeTokens witness

def accountingTokens
    (authority : PrefixCostAuthority)
    (witness : PromptCostWitness) : Nat :=
  match authority with
  | .postHocHit _ _ => effectiveTokens .verifiedHit witness
  | _ => conservativeTokens witness

theorem conservative_is_planning_admissible (context : PlanningContext) :
    PlanningAdmissible context .conservative := by
  trivial

theorem post_hoc_is_not_planning_admissible
    (context : PlanningContext)
    (prefixKey : ModelPrefixCacheKey)
    (requestDigest : Nat) :
    ¬ PlanningAdmissible context (.postHocHit prefixKey requestDigest) := by
  intro admissible
  exact admissible

theorem prefix_key_drift_rejects_lease
    (context : PlanningContext)
    (prefixKey : ModelPrefixCacheKey)
    (runtimeGeneration requestDigest expiresAt : Nat)
    (drift : context.costContext.prefixKey ≠ prefixKey) :
    ¬ PlanningAdmissible
      context
      (.leasedHit prefixKey runtimeGeneration requestDigest expiresAt) := by
  intro admissible
  exact drift admissible.2.1

theorem runtime_generation_drift_rejects_lease
    (context : PlanningContext)
    (prefixKey : ModelPrefixCacheKey)
    (runtimeGeneration requestDigest expiresAt : Nat)
    (drift : context.runtimeGeneration ≠ runtimeGeneration) :
    ¬ PlanningAdmissible
      context
      (.leasedHit prefixKey runtimeGeneration requestDigest expiresAt) := by
  intro admissible
  exact drift admissible.2.2.1

theorem request_drift_rejects_lease
    (context : PlanningContext)
    (prefixKey : ModelPrefixCacheKey)
    (runtimeGeneration requestDigest expiresAt : Nat)
    (drift : context.requestDigest ≠ requestDigest) :
    ¬ PlanningAdmissible
      context
      (.leasedHit prefixKey runtimeGeneration requestDigest expiresAt) := by
  intro admissible
  exact drift admissible.2.2.2.1

theorem expired_lease_is_rejected
    (context : PlanningContext)
    (prefixKey : ModelPrefixCacheKey)
    (runtimeGeneration requestDigest expiresAt : Nat)
    (expired : expiresAt < context.now) :
    ¬ PlanningAdmissible
      context
      (.leasedHit prefixKey runtimeGeneration requestDigest expiresAt) := by
  intro admissible
  exact (Nat.not_le_of_gt expired) admissible.2.2.2.2

theorem prefix_state_drift_rejects_lease
    (context : PlanningContext)
    (prefixKey : ModelPrefixCacheKey)
    (runtimeGeneration requestDigest expiresAt : Nat)
    (drift : context.costContext.prefixState ≠ .verifiedHit) :
    ¬ PlanningAdmissible
      context
      (.leasedHit prefixKey runtimeGeneration requestDigest expiresAt) := by
  intro admissible
  exact drift admissible.1

def examplePlanningContext : PlanningContext :=
  {
    costContext := exampleHitContext
    runtimeGeneration := 7
    requestDigest := 42
    now := 4
  }

def exampleHitLease : PrefixCostAuthority :=
  .leasedHit examplePrefixKey 7 42 8

def examplePostHocHit : PrefixCostAuthority :=
  .postHocHit examplePrefixKey 99

theorem example_hit_lease_is_planning_admissible :
    PlanningAdmissible examplePlanningContext exampleHitLease := by
  decide

theorem example_post_hoc_hit_is_accounting_admissible :
    AccountingAdmissible examplePlanningContext 99 examplePostHocHit := by
  decide

theorem post_hoc_route_a_planning_cost_is_conservative :
    planningTokens examplePostHocHit routeAWitness = 100 := by
  decide

theorem post_hoc_route_a_accounting_cost_is_observed_hit :
    accountingTokens examplePostHocHit routeAWitness = 10 := by
  decide

theorem post_hoc_observation_does_not_lower_prior_planning_cost :
    planningTokens examplePostHocHit routeAWitness = 100 ∧
      accountingTokens examplePostHocHit routeAWitness = 10 := by
  exact
    ⟨
      post_hoc_route_a_planning_cost_is_conservative,
      post_hoc_route_a_accounting_cost_is_observed_hit
    ⟩

end ASPProof.SearchRouteCachePlanningLease
