-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.SearchRouteProviderBatchRealization

structure BatchRealizationCertificate (Route Call : Type) where
  discoveredRoutes : List Route
  scheduledCalls : List Call
  realizes : Call → Route → Prop
  distinctRoutes : discoveredRoutes.Nodup
  covers :
    ∀ route,
      route ∈ discoveredRoutes →
      ∃ call, call ∈ scheduledCalls ∧ realizes call route
  sound :
    ∀ call route,
      call ∈ scheduledCalls →
      realizes call route →
      route ∈ discoveredRoutes

def UniqueProgressCredit
    {Route : Type}
    (routes : List Route)
    (credit : Nat) : Prop :=
  routes.Nodup ∧ routes.length = credit

theorem certified_discovered_route_has_call_witness
    {Route Call : Type}
    (certificate : BatchRealizationCertificate Route Call)
    {route : Route}
    (discovered : route ∈ certificate.discoveredRoutes) :
    ∃ call,
      call ∈ certificate.scheduledCalls ∧
      certificate.realizes call route :=
  certificate.covers route discovered

theorem certified_realized_route_is_discovered
    {Route Call : Type}
    (certificate : BatchRealizationCertificate Route Call)
    {call : Call}
    {route : Route}
    (scheduled : call ∈ certificate.scheduledCalls)
    (realized : certificate.realizes call route) :
    route ∈ certificate.discoveredRoutes :=
  certificate.sound call route scheduled realized

theorem certificate_earns_unique_progress_credit
    {Route Call : Type}
    (certificate : BatchRealizationCertificate Route Call) :
    UniqueProgressCredit
      certificate.discoveredRoutes
      certificate.discoveredRoutes.length :=
  ⟨certificate.distinctRoutes, rfl⟩

inductive ExampleRoute where
  | alpha
  | beta
  deriving DecidableEq

inductive ExampleCall where
  | bundled
  deriving DecidableEq

def distinctRoutes : List ExampleRoute :=
  [.alpha, .beta]

def bundledRealizes (_ : ExampleCall) (_ : ExampleRoute) : Prop :=
  True

def bundledCertificate :
    BatchRealizationCertificate ExampleRoute ExampleCall where
  discoveredRoutes := distinctRoutes
  scheduledCalls := [.bundled]
  realizes := bundledRealizes
  distinctRoutes := by simp [distinctRoutes]
  covers := by
    intro route _
    exact ⟨.bundled, by simp, trivial⟩
  sound := by
    intro _ route _ _
    cases route <;> simp [distinctRoutes]

theorem bundled_certificate_covers_alpha :
    ∃ call,
      call ∈ bundledCertificate.scheduledCalls ∧
      bundledCertificate.realizes call .alpha :=
  certified_discovered_route_has_call_witness
    bundledCertificate
    (by simp [bundledCertificate, distinctRoutes])

theorem bundled_certificate_covers_beta :
    ∃ call,
      call ∈ bundledCertificate.scheduledCalls ∧
      bundledCertificate.realizes call .beta :=
  certified_discovered_route_has_call_witness
    bundledCertificate
    (by simp [bundledCertificate, distinctRoutes])

theorem one_call_covers_two_distinct_routes :
    bundledCertificate.scheduledCalls.length = 1 ∧
    bundledCertificate.discoveredRoutes.length = 2 ∧
    UniqueProgressCredit bundledCertificate.discoveredRoutes 2 := by
  exact
    ⟨rfl, rfl, certificate_earns_unique_progress_credit bundledCertificate⟩

structure ProviderBatchCost where
  uniqueRoutes : Nat
  toolCalls : Nat
  callTokens : Nat
  synthesisTokens : Nat
  deriving DecidableEq

def ProviderBatchCost.totalTokens (cost : ProviderBatchCost) : Nat :=
  cost.callTokens + cost.synthesisTokens

def ValidProviderBatchCost
    {Route Call : Type}
    (certificate : BatchRealizationCertificate Route Call)
    (cost : ProviderBatchCost) : Prop :=
  cost.uniqueRoutes = certificate.discoveredRoutes.length ∧
  cost.toolCalls = certificate.scheduledCalls.length ∧
  (certificate.scheduledCalls ≠ [] → 0 < cost.callTokens)

def bundledCost : ProviderBatchCost where
  uniqueRoutes := 2
  toolCalls := 1
  callTokens := 20
  synthesisTokens := 5

theorem bundled_batch_cost_is_valid :
    ValidProviderBatchCost bundledCertificate bundledCost := by
  exact ⟨rfl, rfl, fun _ => by decide⟩

theorem one_call_can_realize_multiple_routes :
    bundledCost.toolCalls < bundledCost.uniqueRoutes ∧
    bundledCost.totalTokens = 25 := by
  exact ⟨by decide, rfl⟩

def duplicateClaims : List ExampleRoute :=
  [.alpha, .alpha]

theorem count_equality_does_not_imply_identity_coverage :
    duplicateClaims.length = distinctRoutes.length ∧
    ¬ ∀ route,
      route ∈ distinctRoutes →
      route ∈ duplicateClaims := by
  constructor
  · rfl
  · intro coversAll
    have betaCovered :=
      coversAll ExampleRoute.beta (by simp [distinctRoutes])
    simp [duplicateClaims] at betaCovered

theorem duplicate_claims_cannot_earn_two_credits :
    ¬ UniqueProgressCredit duplicateClaims 2 := by
  intro credit
  exact (by simpa [duplicateClaims] using credit.1)

def duplicateRealizes
    (_ : ExampleCall) : ExampleRoute → Prop
  | .alpha => True
  | .beta => False

theorem duplicate_receipt_has_no_beta_witness :
    ¬ ∃ call,
      call ∈ [ExampleCall.bundled] ∧
      duplicateRealizes call .beta := by
  intro witness
  rcases witness with ⟨call, _, realized⟩
  cases call
  exact realized

theorem duplicate_relation_cannot_cover_distinct_routes :
    ¬ ∀ route,
      route ∈ distinctRoutes →
      ∃ call,
        call ∈ [ExampleCall.bundled] ∧
        duplicateRealizes call route := by
  intro coversAll
  exact duplicate_receipt_has_no_beta_witness
    (coversAll ExampleRoute.beta (by simp [distinctRoutes]))

end ASPProof.SearchRouteProviderBatchRealization
