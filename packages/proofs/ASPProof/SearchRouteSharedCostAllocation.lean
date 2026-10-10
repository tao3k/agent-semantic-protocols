-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.SearchRouteSharedCostAllocation

structure SharedCostAllocationCertificate (Route : Type) where
  graphGeneration : Nat
  allocationPolicyId : Nat
  searchCacheContext : Nat
  modelPrefixCacheContext : Nat
  routes : List Route
  distinctRoutes : routes.Nodup
  allocatedTokens : Route → Nat
  sharedTokens : Nat
  callTokens : Nat
  synthesisTokens : Nat
  conserves :
    (routes.map allocatedTokens).sum + sharedTokens =
      callTokens + synthesisTokens

def SharedCostAllocationCertificate.totalTokens
    {Route : Type}
    (certificate : SharedCostAllocationCertificate Route) : Nat :=
  certificate.callTokens + certificate.synthesisTokens

def CostContextAligned
    {Route : Type}
    (left right : SharedCostAllocationCertificate Route) : Prop :=
  left.graphGeneration = right.graphGeneration ∧
  left.allocationPolicyId = right.allocationPolicyId ∧
  left.searchCacheContext = right.searchCacheContext ∧
  left.modelPrefixCacheContext = right.modelPrefixCacheContext

theorem certificate_total_cost_is_conserved
    {Route : Type}
    (certificate : SharedCostAllocationCertificate Route) :
    (certificate.routes.map certificate.allocatedTokens).sum +
        certificate.sharedTokens =
      certificate.totalTokens :=
  certificate.conserves

theorem certificate_routes_are_distinct
    {Route : Type}
    (certificate : SharedCostAllocationCertificate Route) :
    certificate.routes.Nodup :=
  certificate.distinctRoutes

theorem aligned_cost_context_preserves_all_identities
    {Route : Type}
    {left right : SharedCostAllocationCertificate Route}
    (aligned : CostContextAligned left right) :
    left.graphGeneration = right.graphGeneration ∧
    left.allocationPolicyId = right.allocationPolicyId ∧
    left.searchCacheContext = right.searchCacheContext ∧
    left.modelPrefixCacheContext = right.modelPrefixCacheContext :=
  aligned

inductive ExampleRoute where
  | alpha
  | beta
  deriving DecidableEq

def exampleRoutes : List ExampleRoute :=
  [.alpha, .beta]

def balancedAllocation : ExampleRoute → Nat
  | .alpha => 12
  | .beta => 12

def balancedCertificate :
    SharedCostAllocationCertificate ExampleRoute where
  graphGeneration := 7
  allocationPolicyId := 1
  searchCacheContext := 11
  modelPrefixCacheContext := 13
  routes := exampleRoutes
  distinctRoutes := by simp [exampleRoutes]
  allocatedTokens := balancedAllocation
  sharedTokens := 1
  callTokens := 20
  synthesisTokens := 5
  conserves := by decide

theorem balanced_certificate_conserves_complete_cost :
    (balancedCertificate.routes.map
        balancedCertificate.allocatedTokens).sum +
        balancedCertificate.sharedTokens =
      balancedCertificate.callTokens +
        balancedCertificate.synthesisTokens :=
  balancedCertificate.conserves

theorem indivisible_remainder_is_explicit :
    (balancedCertificate.routes.map
        balancedCertificate.allocatedTokens).sum = 24 ∧
    balancedCertificate.sharedTokens = 1 ∧
    balancedCertificate.totalTokens = 25 := by
  exact ⟨rfl, rfl, rfl⟩

theorem full_cost_per_route_double_charges :
    25 + 25 > balancedCertificate.totalTokens ∧
    ¬ 25 + 25 = balancedCertificate.totalTokens := by
  exact ⟨by decide, by decide⟩

theorem omitted_remainder_underreports :
    12 + 12 < balancedCertificate.totalTokens ∧
    ¬ 12 + 12 = balancedCertificate.totalTokens := by
  exact ⟨by decide, by decide⟩

def alphaFavoredAllocation : ExampleRoute → Nat
  | .alpha => 10
  | .beta => 15

def betaFavoredAllocation : ExampleRoute → Nat
  | .alpha => 15
  | .beta => 10

def alphaFavoredCertificate :
    SharedCostAllocationCertificate ExampleRoute where
  graphGeneration := 7
  allocationPolicyId := 2
  searchCacheContext := 11
  modelPrefixCacheContext := 13
  routes := exampleRoutes
  distinctRoutes := by simp [exampleRoutes]
  allocatedTokens := alphaFavoredAllocation
  sharedTokens := 0
  callTokens := 20
  synthesisTokens := 5
  conserves := by decide

def betaFavoredCertificate :
    SharedCostAllocationCertificate ExampleRoute where
  graphGeneration := 7
  allocationPolicyId := 3
  searchCacheContext := 11
  modelPrefixCacheContext := 13
  routes := exampleRoutes
  distinctRoutes := by simp [exampleRoutes]
  allocatedTokens := betaFavoredAllocation
  sharedTokens := 0
  callTokens := 20
  synthesisTokens := 5
  conserves := by decide

theorem equal_total_cost_does_not_determine_route_allocation :
    alphaFavoredCertificate.totalTokens =
        betaFavoredCertificate.totalTokens ∧
    alphaFavoredCertificate.allocatedTokens .alpha ≠
        betaFavoredCertificate.allocatedTokens .alpha := by
  exact ⟨rfl, by decide⟩

theorem different_allocation_policies_are_not_comparable :
    ¬ CostContextAligned
      alphaFavoredCertificate
      betaFavoredCertificate := by
  simp
    [ CostContextAligned
    , alphaFavoredCertificate
    , betaFavoredCertificate
    ]

def staleSearchCacheCertificate :
    SharedCostAllocationCertificate ExampleRoute where
  graphGeneration := 7
  allocationPolicyId := 2
  searchCacheContext := 99
  modelPrefixCacheContext := 13
  routes := exampleRoutes
  distinctRoutes := by simp [exampleRoutes]
  allocatedTokens := alphaFavoredAllocation
  sharedTokens := 0
  callTokens := 20
  synthesisTokens := 5
  conserves := by decide

theorem cache_context_mismatch_rejects_comparison :
    ¬ CostContextAligned
      alphaFavoredCertificate
      staleSearchCacheCertificate := by
  simp
    [ CostContextAligned
    , alphaFavoredCertificate
    , staleSearchCacheCertificate
    ]

theorem allocation_policy_changes_route_pareto_result :
    alphaFavoredCertificate.allocatedTokens .alpha < 13 ∧
    13 < betaFavoredCertificate.allocatedTokens .alpha := by
  exact ⟨by decide, by decide⟩

theorem certificate_is_comparable_with_itself :
    CostContextAligned balancedCertificate balancedCertificate :=
  ⟨rfl, rfl, rfl, rfl⟩

end ASPProof.SearchRouteSharedCostAllocation
