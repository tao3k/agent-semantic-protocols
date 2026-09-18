-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import Mathlib

namespace ASPProof.AXLE.SearchRouteRiskBoundedBatchAdmissionCost

def eventRiskNumerator (failureNumerator : Nat) : Nat :=
  failureNumerator

def fallbackMassNumerator
    (claimCount failureNumerator : Nat) : Nat :=
  claimCount * failureNumerator

theorem nonempty_batch_event_risk_le_fallback_mass
    {claimCount failureNumerator : Nat}
    (hclaims : 0 < claimCount) :
    eventRiskNumerator failureNumerator ≤
      fallbackMassNumerator claimCount failureNumerator := by
  simp [eventRiskNumerator, fallbackMassNumerator]
  nlinarith

theorem multi_claim_positive_risk_is_strict
    {claimCount failureNumerator : Nat}
    (hclaims : 1 < claimCount)
    (hrisk : 0 < failureNumerator) :
    eventRiskNumerator failureNumerator <
      fallbackMassNumerator claimCount failureNumerator := by
  simp [eventRiskNumerator, fallbackMassNumerator]
  nlinarith

def scaledExpectedRoundHalf
    (denominator batchCount fallbackMass : Nat) : Nat :=
  denominator * batchCount + fallbackMass

def scaledIndividualRoundHalf
    (denominator claimCount : Nat) : Nat :=
  denominator * claimCount

def realizedAdaptiveRounds
    (batchCount fallbackCount : Nat) : Nat :=
  2 * batchCount + 2 * fallbackCount

def individualRounds (claimCount : Nat) : Nat :=
  2 * claimCount

def adaptiveInputTokens
    (sharedTokens perClaimTokens claimCount batchCount fallbackCount : Nat) : Nat :=
  batchCount * sharedTokens + claimCount * perClaimTokens +
    fallbackCount * (sharedTokens + perClaimTokens)

def individualInputTokens
    (sharedTokens perClaimTokens claimCount : Nat) : Nat :=
  claimCount * (sharedTokens + perClaimTokens)

theorem concrete_expected_round_admission :
    scaledExpectedRoundHalf 100 2 100 ≤
      scaledIndividualRoundHalf 100 8 := by
  norm_num [scaledExpectedRoundHalf, scaledIndividualRoundHalf]

theorem expected_admission_does_not_imply_realized_round_safety :
    scaledExpectedRoundHalf 100 2 100 ≤
        scaledIndividualRoundHalf 100 8 ∧
    ¬ realizedAdaptiveRounds 2 8 ≤ individualRounds 8 := by
  norm_num [scaledExpectedRoundHalf, scaledIndividualRoundHalf,
    realizedAdaptiveRounds, individualRounds]

def hardFallbackSafe
    (sharedTokens perClaimTokens claimCount batchCount hardFallbackCap : Nat) : Prop :=
  batchCount + hardFallbackCap ≤ claimCount ∧
  (batchCount + hardFallbackCap) * sharedTokens +
      hardFallbackCap * perClaimTokens ≤ claimCount * sharedTokens

theorem fallback_within_hard_cap_is_round_safe
    {sharedTokens perClaimTokens claimCount batchCount hardFallbackCap
      realizedFallbackCount : Nat}
    (hcap : hardFallbackSafe sharedTokens perClaimTokens claimCount batchCount
      hardFallbackCap)
    (hrealized : realizedFallbackCount ≤ hardFallbackCap) :
    realizedAdaptiveRounds batchCount realizedFallbackCount ≤
      individualRounds claimCount := by
  simp [realizedAdaptiveRounds, individualRounds]
  simp [hardFallbackSafe] at hcap
  omega

theorem fallback_within_hard_cap_is_token_safe
    {sharedTokens perClaimTokens claimCount batchCount hardFallbackCap
      realizedFallbackCount : Nat}
    (hcap : hardFallbackSafe sharedTokens perClaimTokens claimCount batchCount
      hardFallbackCap)
    (hrealized : realizedFallbackCount ≤ hardFallbackCap) :
    adaptiveInputTokens sharedTokens perClaimTokens claimCount batchCount
        realizedFallbackCount ≤
      individualInputTokens sharedTokens perClaimTokens claimCount := by
  have hbatch : batchCount + realizedFallbackCount ≤
      batchCount + hardFallbackCap := Nat.add_le_add_left hrealized batchCount
  have hshared := Nat.mul_le_mul_right sharedTokens hbatch
  have hclaim := Nat.mul_le_mul_right perClaimTokens hrealized
  have hcore :
      (batchCount + realizedFallbackCount) * sharedTokens +
          realizedFallbackCount * perClaimTokens ≤ claimCount * sharedTokens :=
    Nat.le_trans (Nat.add_le_add hshared hclaim) hcap.2
  calc
    adaptiveInputTokens sharedTokens perClaimTokens claimCount batchCount
        realizedFallbackCount =
        ((batchCount + realizedFallbackCount) * sharedTokens +
          realizedFallbackCount * perClaimTokens) +
          claimCount * perClaimTokens := by
            simp [adaptiveInputTokens]
            ring
    _ ≤ claimCount * sharedTokens + claimCount * perClaimTokens :=
      Nat.add_le_add_right hcore (claimCount * perClaimTokens)
    _ = individualInputTokens sharedTokens perClaimTokens claimCount := by
      simp [individualInputTokens]
      ring

theorem zero_risk_singleton_fragmentation_has_no_expected_savings
    (denominator claimCount : Nat) :
    scaledExpectedRoundHalf denominator claimCount 0 =
      scaledIndividualRoundHalf denominator claimCount := by
  simp [scaledExpectedRoundHalf, scaledIndividualRoundHalf]

inductive FailureReason where
  | batchTransportUnavailable
  | batchResourceLimit
  | semanticRejection
  | policyMismatch
  | replayMismatch
  | agentPreference
  deriving DecidableEq

def fallbackEligibleReason : FailureReason → Bool
  | .batchTransportUnavailable => true
  | .batchResourceLimit => true
  | .semanticRejection => false
  | .policyMismatch => false
  | .replayMismatch => false
  | .agentPreference => false

theorem semantic_and_agent_failures_are_excluded_from_risk_mass :
    fallbackEligibleReason .semanticRejection = false ∧
    fallbackEligibleReason .policyMismatch = false ∧
    fallbackEligibleReason .replayMismatch = false ∧
    fallbackEligibleReason .agentPreference = false := by
  decide

end ASPProof.AXLE.SearchRouteRiskBoundedBatchAdmissionCost
