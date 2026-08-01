import Mathlib

namespace ASPProof.AXLE.SearchRouteRiskBoundedBatchAdmissionCost

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

theorem expected_admission_does_not_imply_realized_round_safety :
    scaledExpectedRoundHalf 100 2 100 ≤
        scaledIndividualRoundHalf 100 8 ∧
    ¬ realizedAdaptiveRounds 2 8 ≤ individualRounds 8 := by
  sorry

end ASPProof.AXLE.SearchRouteRiskBoundedBatchAdmissionCost
