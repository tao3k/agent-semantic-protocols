-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteAdaptiveBatchFailureIsolation

namespace ASPProof.SearchRouteRiskBoundedBatchAdmission

open ASPProof.SearchRouteBatchClaimDagEquivalence
open ASPProof.SearchRouteAdaptiveBatchFailureIsolation

structure RiskCell where
  batchDigest : Nat
  claimCount : Nat
  failureNumerator : Nat
  reason : FailureReason
  deriving DecidableEq, Repr

def RiskCellWellFormed (denominator : Nat) (cell : RiskCell) : Prop :=
  0 < denominator ∧
  0 < cell.claimCount ∧
  cell.failureNumerator ≤ denominator ∧
  FallbackEligibleReason cell.reason = true

def eventFailureNumerator (cells : List RiskCell) : Nat :=
  (cells.map RiskCell.failureNumerator).sum

def expectedFallbackMassNumerator (cells : List RiskCell) : Nat :=
  (cells.map fun cell => cell.claimCount * cell.failureNumerator).sum

def coveredClaimCount (cells : List RiskCell) : Nat :=
  (cells.map RiskCell.claimCount).sum

theorem event_failure_numerator_le_fallback_mass
    (cells : List RiskCell)
    (hnonempty : ∀ cell ∈ cells, 0 < cell.claimCount) :
    eventFailureNumerator cells ≤ expectedFallbackMassNumerator cells := by
  induction cells with
  | nil => simp [eventFailureNumerator, expectedFallbackMassNumerator]
  | cons head tail ih =>
      have hhead : 0 < head.claimCount := hnonempty head (by simp)
      have htail : ∀ cell ∈ tail, 0 < cell.claimCount := by
        intro cell hcell
        exact hnonempty cell (by simp [hcell])
      have hheadLe : head.failureNumerator ≤
          head.claimCount * head.failureNumerator := by
        calc
          head.failureNumerator = 1 * head.failureNumerator := by simp
          _ ≤ head.claimCount * head.failureNumerator := by
            exact Nat.mul_le_mul_right head.failureNumerator hhead
      simpa [eventFailureNumerator, expectedFallbackMassNumerator] using
        Nat.add_le_add hheadLe (ih htail)

theorem singleton_multi_claim_event_risk_is_strict
    {batchDigest claimCount failureNumerator : Nat}
    {reason : FailureReason}
    (hclaims : 1 < claimCount)
    (hrisk : 0 < failureNumerator) :
    eventFailureNumerator
        [{ batchDigest := batchDigest
           claimCount := claimCount
           failureNumerator := failureNumerator
           reason := reason }] <
      expectedFallbackMassNumerator
        [{ batchDigest := batchDigest
           claimCount := claimCount
           failureNumerator := failureNumerator
           reason := reason }] := by
  simp only [eventFailureNumerator, expectedFallbackMassNumerator, List.map_cons,
    List.map_nil, List.sum_cons, List.sum_nil, Nat.add_zero]
  simpa using Nat.mul_lt_mul_of_pos_right hclaims hrisk

def scaledExpectedRoundHalf
    (denominator batchCount : Nat)
    (cells : List RiskCell) : Nat :=
  denominator * batchCount + expectedFallbackMassNumerator cells

def scaledIndividualRoundHalf (denominator claimCount : Nat) : Nat :=
  denominator * claimCount

def ExpectedRoundAdmissible
    (denominator claimCount batchCount : Nat)
    (cells : List RiskCell) : Prop :=
  scaledExpectedRoundHalf denominator batchCount cells ≤
    scaledIndividualRoundHalf denominator claimCount

def scaledExpectedInputTokens
    (denominator sharedTokens perClaimTokens claimCount batchCount : Nat)
    (cells : List RiskCell) : Nat :=
  denominator * (batchCount * sharedTokens + claimCount * perClaimTokens) +
    expectedFallbackMassNumerator cells * (sharedTokens + perClaimTokens)

def scaledIndividualInputTokens
    (denominator sharedTokens perClaimTokens claimCount : Nat) : Nat :=
  denominator * (claimCount * (sharedTokens + perClaimTokens))

def ExpectedTokenAdmissible
    (denominator sharedTokens perClaimTokens claimCount batchCount : Nat)
    (cells : List RiskCell) : Prop :=
  scaledExpectedInputTokens denominator sharedTokens perClaimTokens
      claimCount batchCount cells ≤
    scaledIndividualInputTokens denominator sharedTokens perClaimTokens claimCount

def ExpectedCostAdmissible
    (denominator sharedTokens perClaimTokens claimCount batchCount : Nat)
    (cells : List RiskCell) : Prop :=
  ExpectedRoundAdmissible denominator claimCount batchCount cells ∧
  ExpectedTokenAdmissible denominator sharedTokens perClaimTokens
    claimCount batchCount cells

def HardFallbackSafe
    (sharedTokens perClaimTokens claimCount batchCount hardFallbackCap : Nat) : Prop :=
  batchCount + hardFallbackCap ≤ claimCount ∧
  (batchCount + hardFallbackCap) * sharedTokens +
      hardFallbackCap * perClaimTokens ≤ claimCount * sharedTokens

theorem fallback_within_hard_cap_is_round_safe
    {sharedTokens perClaimTokens claimCount batchCount hardFallbackCap
      realizedFallbackCount : Nat}
    (hcap : HardFallbackSafe sharedTokens perClaimTokens claimCount batchCount
      hardFallbackCap)
    (hrealized : realizedFallbackCount ≤ hardFallbackCap) :
    adaptiveToolRounds batchCount realizedFallbackCount ≤
      individualToolRounds claimCount := by
  rw [adaptive_tool_rounds_le_individual_iff]
  exact Nat.le_trans (Nat.add_le_add_left hrealized batchCount) hcap.1

theorem fallback_within_hard_cap_is_token_safe
    {sharedTokens perClaimTokens claimCount batchCount hardFallbackCap
      realizedFallbackCount : Nat}
    (hcap : HardFallbackSafe sharedTokens perClaimTokens claimCount batchCount
      hardFallbackCap)
    (hrealized : realizedFallbackCount ≤ hardFallbackCap) :
    adaptiveInputTokens sharedTokens perClaimTokens claimCount batchCount
        realizedFallbackCount ≤
      individualInputTokens sharedTokens perClaimTokens claimCount := by
  rw [adaptive_input_tokens_le_individual_iff]
  have hbatch : batchCount + realizedFallbackCount ≤
      batchCount + hardFallbackCap := Nat.add_le_add_left hrealized batchCount
  have hshared := Nat.mul_le_mul_right sharedTokens hbatch
  have hclaim := Nat.mul_le_mul_right perClaimTokens hrealized
  exact Nat.le_trans (Nat.add_le_add hshared hclaim) hcap.2

structure TailRiskCertificate where
  digest : Nat
  denominator : Nat
  threshold : Nat
  overflowNumerator : Nat
  deriving DecidableEq, Repr

structure ValidTailRiskCertificate
    (allowedOverflowNumerator hardFallbackCap : Nat)
    (certificate : TailRiskCertificate) : Prop where
  denominatorPositive : 0 < certificate.denominator
  numeratorBounded : certificate.overflowNumerator ≤ certificate.denominator
  targetsHardCap : certificate.threshold = hardFallbackCap
  withinPolicy : certificate.overflowNumerator ≤ allowedOverflowNumerator

theorem valid_tail_certificate_targets_hard_cap
    {allowedOverflowNumerator hardFallbackCap : Nat}
    {certificate : TailRiskCertificate}
    (hvalid : ValidTailRiskCertificate allowedOverflowNumerator hardFallbackCap
      certificate) :
    certificate.threshold = hardFallbackCap :=
  hvalid.targetsHardCap

structure RiskEnvelope where
  planDigest : Nat
  checkpointDigest : Nat
  eligibilityPolicyDigest : Nat
  calibrationAuthorityDigest : Nat
  calibrationSnapshotDigest : Nat
  sharedTokens : Nat
  perClaimTokens : Nat
  denominator : Nat
  cells : List RiskCell
  cellsDigest : Nat
  reportedFallbackMassNumerator : Nat
  hardFallbackCap : Nat
  tailCertificate : TailRiskCertificate
  deriving DecidableEq, Repr

structure ValidRiskEnvelope
    (expectedPlanDigest expectedCheckpointDigest expectedEligibilityPolicyDigest : Nat)
    (expectedCalibrationAuthorityDigest expectedCalibrationSnapshotDigest : Nat)
    (sharedTokens perClaimTokens claimCount batchCount allowedTailOverflowNumerator : Nat)
    (envelope : RiskEnvelope) : Prop where
  planBound : envelope.planDigest = expectedPlanDigest
  checkpointBound : envelope.checkpointDigest = expectedCheckpointDigest
  eligibilityPolicyBound :
    envelope.eligibilityPolicyDigest = expectedEligibilityPolicyDigest
  calibrationAuthorityBound :
    envelope.calibrationAuthorityDigest = expectedCalibrationAuthorityDigest
  calibrationSnapshotBound :
    envelope.calibrationSnapshotDigest = expectedCalibrationSnapshotDigest
  sharedTokensBound : envelope.sharedTokens = sharedTokens
  perClaimTokensBound : envelope.perClaimTokens = perClaimTokens
  denominatorPositive : 0 < envelope.denominator
  batchCountExact : envelope.cells.length = batchCount
  claimCountExact : coveredClaimCount envelope.cells = claimCount
  cellsWellFormed :
    ∀ cell ∈ envelope.cells, RiskCellWellFormed envelope.denominator cell
  reportedMassExact : envelope.reportedFallbackMassNumerator =
    expectedFallbackMassNumerator envelope.cells
  expectedCost : ExpectedCostAdmissible envelope.denominator sharedTokens perClaimTokens
    claimCount batchCount envelope.cells
  tail : ValidTailRiskCertificate allowedTailOverflowNumerator
    envelope.hardFallbackCap envelope.tailCertificate
  hardFallback : HardFallbackSafe sharedTokens perClaimTokens claimCount batchCount
    envelope.hardFallbackCap

theorem valid_envelope_reports_claim_weighted_mass
    {planDigest checkpointDigest eligibilityPolicyDigest : Nat}
    {calibrationAuthorityDigest calibrationSnapshotDigest : Nat}
    {sharedTokens perClaimTokens claimCount batchCount allowedTailOverflowNumerator : Nat}
    {envelope : RiskEnvelope}
    (hvalid : ValidRiskEnvelope planDigest checkpointDigest eligibilityPolicyDigest
      calibrationAuthorityDigest calibrationSnapshotDigest sharedTokens perClaimTokens
      claimCount batchCount allowedTailOverflowNumerator envelope) :
    envelope.reportedFallbackMassNumerator =
      expectedFallbackMassNumerator envelope.cells :=
  hvalid.reportedMassExact

theorem valid_envelope_hard_cap_preserves_realized_round_bound
    {planDigest checkpointDigest eligibilityPolicyDigest : Nat}
    {calibrationAuthorityDigest calibrationSnapshotDigest : Nat}
    {sharedTokens perClaimTokens claimCount batchCount allowedTailOverflowNumerator : Nat}
    {envelope : RiskEnvelope}
    (hvalid : ValidRiskEnvelope planDigest checkpointDigest eligibilityPolicyDigest
      calibrationAuthorityDigest calibrationSnapshotDigest sharedTokens perClaimTokens
      claimCount batchCount allowedTailOverflowNumerator envelope)
    {realizedFallbackCount : Nat}
    (hrealized : realizedFallbackCount ≤ envelope.hardFallbackCap) :
    adaptiveToolRounds batchCount realizedFallbackCount ≤
      individualToolRounds claimCount := by
  exact fallback_within_hard_cap_is_round_safe hvalid.hardFallback hrealized

theorem valid_envelope_hard_cap_preserves_realized_token_bound
    {planDigest checkpointDigest eligibilityPolicyDigest : Nat}
    {calibrationAuthorityDigest calibrationSnapshotDigest : Nat}
    {sharedTokens perClaimTokens claimCount batchCount allowedTailOverflowNumerator : Nat}
    {envelope : RiskEnvelope}
    (hvalid : ValidRiskEnvelope planDigest checkpointDigest eligibilityPolicyDigest
      calibrationAuthorityDigest calibrationSnapshotDigest sharedTokens perClaimTokens
      claimCount batchCount allowedTailOverflowNumerator envelope)
    {realizedFallbackCount : Nat}
    (hrealized : realizedFallbackCount ≤ envelope.hardFallbackCap) :
    adaptiveInputTokens sharedTokens perClaimTokens claimCount batchCount
        realizedFallbackCount ≤
      individualInputTokens sharedTokens perClaimTokens claimCount := by
  exact fallback_within_hard_cap_is_token_safe hvalid.hardFallback hrealized

def transportRiskCell : RiskCell :=
  { batchDigest := 501
    claimCount := 4
    failureNumerator := 25
    reason := .batchTransportUnavailable }

def zeroResourceRiskCell : RiskCell :=
  { batchDigest := 502
    claimCount := 4
    failureNumerator := 0
    reason := .batchResourceLimit }

def exampleRiskCells : List RiskCell :=
  [transportRiskCell, zeroResourceRiskCell]

theorem example_event_count_understates_fallback_mass :
    eventFailureNumerator exampleRiskCells = 25 ∧
    expectedFallbackMassNumerator exampleRiskCells = 100 := by
  decide

theorem example_expected_cost_is_admissible :
    ExpectedCostAdmissible 100 10 2 8 2 exampleRiskCells := by
  simp [ExpectedCostAdmissible, ExpectedRoundAdmissible,
    ExpectedTokenAdmissible, scaledExpectedRoundHalf,
    scaledIndividualRoundHalf, scaledExpectedInputTokens,
    scaledIndividualInputTokens, expectedFallbackMassNumerator,
    exampleRiskCells, transportRiskCell, zeroResourceRiskCell]

theorem expected_admission_does_not_imply_realized_round_safety :
    ExpectedCostAdmissible 100 10 2 8 2 exampleRiskCells ∧
    ¬ adaptiveToolRounds 2 8 ≤ individualToolRounds 8 := by
  constructor
  · exact example_expected_cost_is_admissible
  · decide

theorem example_hard_cap_closes_realized_round_bound :
    HardFallbackSafe 10 2 8 2 5 ∧
    adaptiveToolRounds 2 5 ≤ individualToolRounds 8 := by
  constructor
  · simp [HardFallbackSafe]
  · decide

theorem example_hard_cap_closes_realized_token_bound :
    HardFallbackSafe 10 2 8 2 5 ∧
    adaptiveInputTokens 10 2 8 2 5 ≤ individualInputTokens 10 2 8 := by
  constructor
  · simp [HardFallbackSafe]
  · decide

theorem zero_risk_singleton_fragmentation_has_no_expected_round_savings
    (denominator claimCount : Nat) :
    scaledExpectedRoundHalf denominator claimCount [] =
      scaledIndividualRoundHalf denominator claimCount := by
  simp [scaledExpectedRoundHalf, scaledIndividualRoundHalf,
    expectedFallbackMassNumerator]

def semanticRiskCell : RiskCell :=
  { batchDigest := 503
    claimCount := 4
    failureNumerator := 25
    reason := .semanticRejection }

theorem semantic_rejection_cannot_form_risk_cell :
    ¬ RiskCellWellFormed 100 semanticRiskCell := by
  simp [RiskCellWellFormed, semanticRiskCell, FallbackEligibleReason]

def agentPreferenceRiskCell : RiskCell :=
  { batchDigest := 504
    claimCount := 1
    failureNumerator := 1
    reason := .agentPreference }

theorem agent_preference_cannot_form_risk_cell :
    ¬ RiskCellWellFormed 100 agentPreferenceRiskCell := by
  simp [RiskCellWellFormed, agentPreferenceRiskCell, FallbackEligibleReason]

structure RiskCacheIdentity where
  planDigest : Nat
  checkpointDigest : Nat
  eligibilityPolicyDigest : Nat
  calibrationAuthorityDigest : Nat
  calibrationSnapshotDigest : Nat
  sharedTokens : Nat
  perClaimTokens : Nat
  denominator : Nat
  cellsDigest : Nat
  hardFallbackCap : Nat
  allowedTailOverflowNumerator : Nat
  tailCertificateDigest : Nat
  deriving DecidableEq, Repr

def PlanOnlyRiskCacheHit (left right : RiskCacheIdentity) : Prop :=
  left.planDigest = right.planDigest

def RiskCacheReusable (left right : RiskCacheIdentity) : Prop :=
  left = right

def currentRiskIdentity : RiskCacheIdentity :=
  { planDigest := 601
    checkpointDigest := 602
    eligibilityPolicyDigest := 603
    calibrationAuthorityDigest := 604
    calibrationSnapshotDigest := 605
    sharedTokens := 10
    perClaimTokens := 2
    denominator := 100
    cellsDigest := 606
    hardFallbackCap := 5
    allowedTailOverflowNumerator := 1
    tailCertificateDigest := 607 }

def staleCalibrationRiskIdentity : RiskCacheIdentity :=
  { planDigest := 601
    checkpointDigest := 602
    eligibilityPolicyDigest := 603
    calibrationAuthorityDigest := 604
    calibrationSnapshotDigest := 608
    sharedTokens := 10
    perClaimTokens := 2
    denominator := 100
    cellsDigest := 609
    hardFallbackCap := 5
    allowedTailOverflowNumerator := 1
    tailCertificateDigest := 610 }

def changedSearchCostRiskIdentity : RiskCacheIdentity :=
  { planDigest := 601
    checkpointDigest := 602
    eligibilityPolicyDigest := 603
    calibrationAuthorityDigest := 604
    calibrationSnapshotDigest := 605
    sharedTokens := 20
    perClaimTokens := 3
    denominator := 100
    cellsDigest := 606
    hardFallbackCap := 5
    allowedTailOverflowNumerator := 1
    tailCertificateDigest := 607 }

theorem plan_only_hit_does_not_prove_risk_cache_reuse :
    PlanOnlyRiskCacheHit currentRiskIdentity staleCalibrationRiskIdentity ∧
    ¬ RiskCacheReusable currentRiskIdentity staleCalibrationRiskIdentity := by
  simp [PlanOnlyRiskCacheHit, RiskCacheReusable, currentRiskIdentity,
    staleCalibrationRiskIdentity]

theorem cost_profile_change_invalidates_risk_cache_reuse :
    PlanOnlyRiskCacheHit currentRiskIdentity changedSearchCostRiskIdentity ∧
    ¬ RiskCacheReusable currentRiskIdentity changedSearchCostRiskIdentity := by
  simp [PlanOnlyRiskCacheHit, RiskCacheReusable, currentRiskIdentity,
    changedSearchCostRiskIdentity]

structure ModelPrefixCacheIdentity where
  modelDigest : Nat
  tokenizerDigest : Nat
  prefixDigest : Nat
  deriving DecidableEq, Repr

def ModelPrefixCacheHit
    (left right : ModelPrefixCacheIdentity) : Prop :=
  left = right

def exampleModelPrefixIdentity : ModelPrefixCacheIdentity :=
  { modelDigest := 701
    tokenizerDigest := 702
    prefixDigest := 703 }

theorem model_prefix_hit_does_not_prove_risk_cache_reuse :
    ModelPrefixCacheHit exampleModelPrefixIdentity exampleModelPrefixIdentity ∧
    ¬ RiskCacheReusable currentRiskIdentity staleCalibrationRiskIdentity := by
  simp [ModelPrefixCacheHit, RiskCacheReusable, exampleModelPrefixIdentity,
    currentRiskIdentity, staleCalibrationRiskIdentity]

end ASPProof.SearchRouteRiskBoundedBatchAdmission
