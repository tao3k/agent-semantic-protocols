-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteRiskBoundedBatchAdmission
import ASPProof.SearchRouteGraphRouterParetoCostSelection

namespace ASPProof.SearchRouteRiskFeasibleGraphSelection

open ASPProof.SearchRouteRiskBoundedBatchAdmission
open ASPProof.SearchRouteGraphRouterParetoCostSelection
open ASPProof.SearchRouteAdaptiveBatchFailureIsolation

structure RiskRouteCandidate where
  routeId : Nat
  cost : RouteCost
  routeIdentity : RouteEvaluationIdentity
  riskIdentity : RiskCacheIdentity
  expectedPlanDigest : Nat
  expectedCheckpointDigest : Nat
  expectedEligibilityPolicyDigest : Nat
  expectedCalibrationAuthorityDigest : Nat
  expectedCalibrationSnapshotDigest : Nat
  sharedTokens : Nat
  perClaimTokens : Nat
  claimCount : Nat
  batchCount : Nat
  allowedTailOverflowNumerator : Nat
  envelope : RiskEnvelope

def RiskIdentityMatchesEnvelope (candidate : RiskRouteCandidate) : Prop :=
  candidate.riskIdentity.planDigest = candidate.envelope.planDigest ∧
  candidate.riskIdentity.checkpointDigest = candidate.envelope.checkpointDigest ∧
  candidate.riskIdentity.eligibilityPolicyDigest =
    candidate.envelope.eligibilityPolicyDigest ∧
  candidate.riskIdentity.calibrationAuthorityDigest =
    candidate.envelope.calibrationAuthorityDigest ∧
  candidate.riskIdentity.calibrationSnapshotDigest =
    candidate.envelope.calibrationSnapshotDigest ∧
  candidate.riskIdentity.sharedTokens = candidate.envelope.sharedTokens ∧
  candidate.riskIdentity.perClaimTokens = candidate.envelope.perClaimTokens ∧
  candidate.riskIdentity.denominator = candidate.envelope.denominator ∧
  candidate.riskIdentity.cellsDigest = candidate.envelope.cellsDigest ∧
  candidate.riskIdentity.hardFallbackCap = candidate.envelope.hardFallbackCap ∧
  candidate.riskIdentity.allowedTailOverflowNumerator =
    candidate.allowedTailOverflowNumerator ∧
  candidate.riskIdentity.tailCertificateDigest =
    candidate.envelope.tailCertificate.digest

def RiskFeasible (candidate : RiskRouteCandidate) : Prop :=
  ValidRiskEnvelope candidate.expectedPlanDigest candidate.expectedCheckpointDigest
      candidate.expectedEligibilityPolicyDigest
      candidate.expectedCalibrationAuthorityDigest
      candidate.expectedCalibrationSnapshotDigest
      candidate.sharedTokens candidate.perClaimTokens candidate.claimCount
      candidate.batchCount candidate.allowedTailOverflowNumerator candidate.envelope ∧
    RiskIdentityMatchesEnvelope candidate

def RouteFeasible (caps : RouteCost) (candidate : RiskRouteCandidate) : Prop :=
  Feasible candidate.cost caps

def RiskRouteAdmitted (caps : RouteCost) (candidate : RiskRouteCandidate) : Prop :=
  RouteFeasible caps candidate ∧ RiskFeasible candidate

def RiskAdmittedNoWorse
    (caps : RouteCost)
    (left right : RiskRouteCandidate) : Prop :=
  RiskRouteAdmitted caps left ∧
  (¬ RiskRouteAdmitted caps right ∨ NoWorse left.cost right.cost)

theorem admitted_candidate_is_route_and_risk_feasible
    {caps : RouteCost}
    {candidate : RiskRouteCandidate}
    (hadmitted : RiskRouteAdmitted caps candidate) :
    RouteFeasible caps candidate ∧ RiskFeasible candidate :=
  hadmitted

theorem admitted_no_worse_left_is_risk_feasible
    {caps : RouteCost}
    {left right : RiskRouteCandidate}
    (hadmitted : RiskAdmittedNoWorse caps left right) :
    RiskFeasible left :=
  hadmitted.1.2

def safeTailCertificate : TailRiskCertificate :=
  { digest := 607
    denominator := 100
    threshold := 5
    overflowNumerator := 1 }

def unsafeTailCertificate : TailRiskCertificate :=
  { digest := 611
    denominator := 100
    threshold := 6
    overflowNumerator := 1 }

def safeRiskEnvelope : RiskEnvelope :=
  { planDigest := 601
    checkpointDigest := 602
    eligibilityPolicyDigest := 603
    calibrationAuthorityDigest := 604
    calibrationSnapshotDigest := 605
    sharedTokens := 10
    perClaimTokens := 2
    denominator := 100
    cells := exampleRiskCells
    cellsDigest := 606
    reportedFallbackMassNumerator := 100
    hardFallbackCap := 5
    tailCertificate := safeTailCertificate }

def unsafeRiskEnvelope : RiskEnvelope :=
  { planDigest := 601
    checkpointDigest := 602
    eligibilityPolicyDigest := 603
    calibrationAuthorityDigest := 604
    calibrationSnapshotDigest := 605
    sharedTokens := 10
    perClaimTokens := 2
    denominator := 100
    cells := exampleRiskCells
    cellsDigest := 606
    reportedFallbackMassNumerator := 100
    hardFallbackCap := 6
    tailCertificate := unsafeTailCertificate }

def unsafeCapRiskIdentity : RiskCacheIdentity :=
  { planDigest := 601
    checkpointDigest := 602
    eligibilityPolicyDigest := 603
    calibrationAuthorityDigest := 604
    calibrationSnapshotDigest := 605
    sharedTokens := 10
    perClaimTokens := 2
    denominator := 100
    cellsDigest := 606
    hardFallbackCap := 6
    allowedTailOverflowNumerator := 1
    tailCertificateDigest := 611 }

theorem safe_risk_envelope_is_valid :
    ValidRiskEnvelope 601 602 603 604 605 10 2 8 2 1 safeRiskEnvelope := by
  refine {
    planBound := rfl
    checkpointBound := rfl
    eligibilityPolicyBound := rfl
    calibrationAuthorityBound := rfl
    calibrationSnapshotBound := rfl
    sharedTokensBound := rfl
    perClaimTokensBound := rfl
    denominatorPositive := by decide
    batchCountExact := by decide
    claimCountExact := by
      simp [safeRiskEnvelope, coveredClaimCount, exampleRiskCells,
        transportRiskCell, zeroResourceRiskCell]
    cellsWellFormed := by
      intro cell hcell
      simp [safeRiskEnvelope, exampleRiskCells] at hcell
      rcases hcell with rfl | rfl <;>
        simp [RiskCellWellFormed, safeRiskEnvelope, transportRiskCell,
          zeroResourceRiskCell, FallbackEligibleReason]
    reportedMassExact := by
      decide
    expectedCost := example_expected_cost_is_admissible
    tail := {
      denominatorPositive := by decide
      numeratorBounded := by decide
      targetsHardCap := rfl
      withinPolicy := by decide
    }
    hardFallback := by
      simp [HardFallbackSafe, safeRiskEnvelope]
  }

def exampleRouteIdentity : RouteEvaluationIdentity :=
  { workspaceSnapshotDigest := 801
    queryDigest := 802
    searchPolicyDigest := 803
    modelDigest := 804
    promptAndToolDigest := 805
    budgetVectorDigest := 806 }

def routeCaps : RouteCost :=
  { graphHops := 3
    interactionRounds := 3
    projectedTokens := 100
    verificationOps := 3
    searchExecutions := 3
    modelPrefixRecomputations := 3 }

def shortRouteCost : RouteCost :=
  { graphHops := 1
    interactionRounds := 1
    projectedTokens := 20
    verificationOps := 1
    searchExecutions := 1
    modelPrefixRecomputations := 1 }

def cachedShortRouteCost : RouteCost :=
  { graphHops := 1
    interactionRounds := 1
    projectedTokens := 20
    verificationOps := 1
    searchExecutions := 1
    modelPrefixRecomputations := 0 }

def longerRouteCost : RouteCost :=
  { graphHops := 2
    interactionRounds := 1
    projectedTokens := 20
    verificationOps := 1
    searchExecutions := 1
    modelPrefixRecomputations := 1 }

def shortUnsafeCandidate : RiskRouteCandidate :=
  { routeId := 901
    cost := shortRouteCost
    routeIdentity := exampleRouteIdentity
    riskIdentity := unsafeCapRiskIdentity
    expectedPlanDigest := 601
    expectedCheckpointDigest := 602
    expectedEligibilityPolicyDigest := 603
    expectedCalibrationAuthorityDigest := 604
    expectedCalibrationSnapshotDigest := 605
    sharedTokens := 10
    perClaimTokens := 2
    claimCount := 8
    batchCount := 2
    allowedTailOverflowNumerator := 1
    envelope := unsafeRiskEnvelope }

def cachedShortUnsafeCandidate : RiskRouteCandidate :=
  { routeId := 902
    cost := cachedShortRouteCost
    routeIdentity := exampleRouteIdentity
    riskIdentity := unsafeCapRiskIdentity
    expectedPlanDigest := 601
    expectedCheckpointDigest := 602
    expectedEligibilityPolicyDigest := 603
    expectedCalibrationAuthorityDigest := 604
    expectedCalibrationSnapshotDigest := 605
    sharedTokens := 10
    perClaimTokens := 2
    claimCount := 8
    batchCount := 2
    allowedTailOverflowNumerator := 1
    envelope := unsafeRiskEnvelope }

def longerSafeCandidate : RiskRouteCandidate :=
  { routeId := 903
    cost := longerRouteCost
    routeIdentity := exampleRouteIdentity
    riskIdentity := currentRiskIdentity
    expectedPlanDigest := 601
    expectedCheckpointDigest := 602
    expectedEligibilityPolicyDigest := 603
    expectedCalibrationAuthorityDigest := 604
    expectedCalibrationSnapshotDigest := 605
    sharedTokens := 10
    perClaimTokens := 2
    claimCount := 8
    batchCount := 2
    allowedTailOverflowNumerator := 1
    envelope := safeRiskEnvelope }

theorem safe_candidate_risk_identity_matches :
    RiskIdentityMatchesEnvelope longerSafeCandidate := by
  simp [RiskIdentityMatchesEnvelope, longerSafeCandidate, currentRiskIdentity,
    safeRiskEnvelope, safeTailCertificate]

theorem longer_candidate_is_risk_feasible :
    RiskFeasible longerSafeCandidate := by
  exact ⟨safe_risk_envelope_is_valid, safe_candidate_risk_identity_matches⟩

theorem unsafe_cap_satisfies_round_bound_but_fails_token_bound :
    2 + 6 ≤ 8 ∧ ¬ (2 + 6) * 10 + 6 * 2 ≤ 8 * 10 := by
  decide

theorem short_candidate_is_not_risk_feasible :
    ¬ RiskFeasible shortUnsafeCandidate := by
  intro hfeasible
  have htoken := hfeasible.1.hardFallback.2
  have himpossible : (2 + 6) * 10 + 6 * 2 ≤ 8 * 10 := by
    change (2 + 6) * 10 + 6 * 2 ≤ 8 * 10 at htoken
    exact htoken
  exact unsafe_cap_satisfies_round_bound_but_fails_token_bound.2 himpossible

theorem short_candidate_is_route_feasible :
    RouteFeasible routeCaps shortUnsafeCandidate := by
  simp [RouteFeasible, Feasible, NoWorse, routeCaps, shortUnsafeCandidate,
    shortRouteCost]

theorem longer_candidate_is_route_feasible :
    RouteFeasible routeCaps longerSafeCandidate := by
  simp [RouteFeasible, Feasible, NoWorse, routeCaps, longerSafeCandidate,
    longerRouteCost]

theorem hop_first_prefers_short_risk_infeasible_candidate :
    HopFirstLexBetter shortUnsafeCandidate.cost longerSafeCandidate.cost := by
  exact Or.inl (by decide)

theorem hop_first_preference_does_not_imply_risk_admission :
    HopFirstLexBetter shortUnsafeCandidate.cost longerSafeCandidate.cost ∧
    RouteFeasible routeCaps shortUnsafeCandidate ∧
    ¬ RiskFeasible shortUnsafeCandidate ∧
    RiskRouteAdmitted routeCaps longerSafeCandidate := by
  exact ⟨hop_first_prefers_short_risk_infeasible_candidate,
    short_candidate_is_route_feasible,
    short_candidate_is_not_risk_feasible,
    longer_candidate_is_route_feasible,
    longer_candidate_is_risk_feasible⟩

theorem longer_risk_feasible_route_is_admitted_over_shorter_route :
    RiskAdmittedNoWorse routeCaps longerSafeCandidate shortUnsafeCandidate := by
  exact ⟨
    ⟨longer_candidate_is_route_feasible, longer_candidate_is_risk_feasible⟩,
    Or.inl (fun hadmitted => short_candidate_is_not_risk_feasible hadmitted.2)⟩

theorem model_prefix_cost_improvement_does_not_create_risk_admission :
    NoWorse cachedShortUnsafeCandidate.cost shortUnsafeCandidate.cost ∧
    ¬ RiskRouteAdmitted routeCaps cachedShortUnsafeCandidate := by
  constructor
  · simp [NoWorse, cachedShortUnsafeCandidate, shortUnsafeCandidate,
      cachedShortRouteCost, shortRouteCost]
  · intro hadmitted
    exact short_candidate_is_not_risk_feasible hadmitted.2

structure RiskAwareEvaluationIdentity where
  route : RouteEvaluationIdentity
  risk : RiskCacheIdentity

def LegacyRouteComparable
    (left right : RiskAwareEvaluationIdentity) : Prop :=
  RouteEvaluationComparable left.route right.route

def RiskAwareComparable
    (left right : RiskAwareEvaluationIdentity) : Prop :=
  RouteEvaluationComparable left.route right.route ∧
  RiskCacheReusable left.risk right.risk

def currentEvaluationIdentity : RiskAwareEvaluationIdentity :=
  { route := exampleRouteIdentity
    risk := currentRiskIdentity }

def staleCalibrationEvaluationIdentity : RiskAwareEvaluationIdentity :=
  { route := exampleRouteIdentity
    risk := staleCalibrationRiskIdentity }

def changedCostEvaluationIdentity : RiskAwareEvaluationIdentity :=
  { route := exampleRouteIdentity
    risk := changedSearchCostRiskIdentity }

theorem legacy_route_identity_does_not_prove_calibration_comparability :
    LegacyRouteComparable currentEvaluationIdentity staleCalibrationEvaluationIdentity ∧
    ¬ RiskAwareComparable currentEvaluationIdentity
      staleCalibrationEvaluationIdentity := by
  constructor
  · rfl
  · intro hcomparable
    exact plan_only_hit_does_not_prove_risk_cache_reuse.2 hcomparable.2

theorem legacy_route_identity_does_not_prove_cost_profile_comparability :
    LegacyRouteComparable currentEvaluationIdentity changedCostEvaluationIdentity ∧
    ¬ RiskAwareComparable currentEvaluationIdentity changedCostEvaluationIdentity := by
  constructor
  · rfl
  · intro hcomparable
    exact cost_profile_change_invalidates_risk_cache_reuse.2 hcomparable.2

def SelectedFrom
    (catalog : RiskRouteCandidate → Prop)
    (caps : RouteCost)
    (selected : RiskRouteCandidate) : Prop :=
  catalog selected ∧
  RiskRouteAdmitted caps selected ∧
  ∀ candidate, catalog candidate → RiskRouteAdmitted caps candidate →
    NoWorse selected.cost candidate.cost

theorem selected_candidate_is_risk_feasible
    {catalog : RiskRouteCandidate → Prop}
    {caps : RouteCost}
    {selected : RiskRouteCandidate}
    (hselected : SelectedFrom catalog caps selected) :
    RiskFeasible selected :=
  hselected.2.1.2

theorem empty_admitted_catalog_has_no_selection
    {catalog : RiskRouteCandidate → Prop}
    {caps : RouteCost}
    (hempty : ∀ candidate, catalog candidate → ¬ RiskRouteAdmitted caps candidate) :
    ¬ ∃ selected, SelectedFrom catalog caps selected := by
  intro hselected
  rcases hselected with ⟨selected, hselected⟩
  exact hempty selected hselected.1 hselected.2.1

end ASPProof.SearchRouteRiskFeasibleGraphSelection
