-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteAtomicMultiResourceReservation

namespace ASPProof.SearchRoutePerDimensionEffectResolution

open ASPProof.SearchRouteReservationLifecycleConservation
open ASPProof.SearchRouteAtomicMultiResourceReservation

inductive DimensionOutcome where
  | consumed
  | noEffect
  | unknown
  deriving DecidableEq, Repr

structure EffectResolutionVector where
  tokens : DimensionOutcome
  money : DimensionOutcome
  providerQuota : DimensionOutcome
  deriving DecidableEq, Repr

def resolveHeldDimension
    (ledger : LifecycleLedger)
    (amount : Nat)
    (outcome : DimensionOutcome) : LifecycleLedger :=
  match outcome with
  | .consumed => consumeHeld ledger amount
  | .noEffect => releaseHeld ledger amount
  | .unknown => quarantineHeld ledger amount

theorem dimension_resolution_preserves_total
    (ledger : LifecycleLedger)
    (amount : Nat)
    (outcome : DimensionOutcome)
    (enough : amount ≤ ledger.held) :
    ledgerTotal (resolveHeldDimension ledger amount outcome) =
      ledgerTotal ledger := by
  cases outcome with
  | consumed =>
      exact consume_held_preserves_total ledger amount enough
  | noEffect =>
      exact release_held_preserves_total ledger amount enough
  | unknown =>
      exact quarantine_held_preserves_total ledger amount enough

def resolveVector
    (ledger : MultiResourceLedger)
    (demand : ResourceDemand)
    (outcome : EffectResolutionVector) : MultiResourceLedger :=
  {
    tokens :=
      resolveHeldDimension ledger.tokens demand.tokens outcome.tokens
    money :=
      resolveHeldDimension ledger.money demand.money outcome.money
    providerQuota :=
      resolveHeldDimension
        ledger.providerQuota
        demand.providerQuota
        outcome.providerQuota
  }

theorem vector_resolution_preserves_all_totals
    (ledger : MultiResourceLedger)
    (demand : ResourceDemand)
    (outcome : EffectResolutionVector)
    (tokensHeld : demand.tokens ≤ ledger.tokens.held)
    (moneyHeld : demand.money ≤ ledger.money.held)
    (quotaHeld :
      demand.providerQuota ≤ ledger.providerQuota.held) :
    ledgerTotal (resolveVector ledger demand outcome).tokens =
        ledgerTotal ledger.tokens ∧
      ledgerTotal (resolveVector ledger demand outcome).money =
        ledgerTotal ledger.money ∧
      ledgerTotal (resolveVector ledger demand outcome).providerQuota =
        ledgerTotal ledger.providerQuota := by
  exact
    ⟨
      dimension_resolution_preserves_total
        ledger.tokens demand.tokens outcome.tokens tokensHeld,
      dimension_resolution_preserves_total
        ledger.money demand.money outcome.money moneyHeld,
      dimension_resolution_preserves_total
        ledger.providerQuota
        demand.providerQuota
        outcome.providerQuota
        quotaHeld
    ⟩

def resolvableExampleLedger : MultiResourceLedger :=
  {
    tokens := emptyResourceLedger 100
    money := emptyResourceLedger 20
    providerQuota := emptyResourceLedger 2
  }

def resolvableDemand : ResourceDemand :=
  {
    tokens := 20
    money := 10
    providerQuota := 1
  }

def heldExampleLedger : MultiResourceLedger :=
  reserveAll resolvableExampleLedger resolvableDemand

def mixedOutcome : EffectResolutionVector :=
  {
    tokens := .unknown
    money := .consumed
    providerQuota := .consumed
  }

def allConsumedOutcome : EffectResolutionVector :=
  {
    tokens := .consumed
    money := .consumed
    providerQuota := .consumed
  }

def allNoEffectOutcome : EffectResolutionVector :=
  {
    tokens := .noEffect
    money := .noEffect
    providerQuota := .noEffect
  }

def mixedResolvedLedger : MultiResourceLedger :=
  resolveVector heldExampleLedger resolvableDemand mixedOutcome

theorem example_reservation_is_admissible :
    CanReserveAll resolvableExampleLedger resolvableDemand := by
  decide

theorem example_tokens_become_quarantined :
    mixedResolvedLedger.tokens.quarantined = 20 := by
  decide

theorem example_money_becomes_consumed :
    mixedResolvedLedger.money.consumed = 10 := by
  decide

theorem example_provider_quota_becomes_consumed :
    mixedResolvedLedger.providerQuota.consumed = 1 := by
  decide

theorem mixed_outcome_is_not_scalar :
    mixedOutcome ≠ allConsumedOutcome ∧
      mixedOutcome ≠ allNoEffectOutcome := by
  decide

theorem resolved_dimensions_survive_one_unknown_dimension :
    mixedResolvedLedger.tokens.quarantined = 20 ∧
      mixedResolvedLedger.money.consumed = 10 ∧
      mixedResolvedLedger.providerQuota.consumed = 1 := by
  decide

end ASPProof.SearchRoutePerDimensionEffectResolution
