import Mathlib

namespace ASPProof.AXLE.SearchRouteAdaptiveBatchFailureIsolationCost

def individualToolRounds (claimCount : Nat) : Nat :=
  2 * claimCount

def adaptiveToolRounds (batchCount unresolvedCount : Nat) : Nat :=
  2 * batchCount + 2 * unresolvedCount

theorem adaptive_tool_rounds_le_individual_iff
    (claimCount batchCount unresolvedCount : Nat) :
    adaptiveToolRounds batchCount unresolvedCount ≤ individualToolRounds claimCount ↔
      batchCount + unresolvedCount ≤ claimCount := by
  simp [adaptiveToolRounds, individualToolRounds]
  omega

theorem adaptive_tool_rounds_strictly_less_iff
    (claimCount batchCount unresolvedCount : Nat) :
    adaptiveToolRounds batchCount unresolvedCount < individualToolRounds claimCount ↔
      batchCount + unresolvedCount < claimCount := by
  simp [adaptiveToolRounds, individualToolRounds]
  omega

theorem singleton_partition_without_fallback_has_no_round_savings
    (claimCount : Nat) :
    adaptiveToolRounds claimCount 0 = individualToolRounds claimCount := by
  simp [adaptiveToolRounds, individualToolRounds]

theorem singleton_partition_with_any_fallback_regresses
    {claimCount unresolvedCount : Nat}
    (hunresolved : 0 < unresolvedCount) :
    individualToolRounds claimCount < adaptiveToolRounds claimCount unresolvedCount := by
  simp [adaptiveToolRounds, individualToolRounds]
  omega

theorem full_fallback_cannot_satisfy_round_nonregression
    {claimCount batchCount : Nat}
    (hbatch : 0 < batchCount) :
    ¬ adaptiveToolRounds batchCount claimCount ≤ individualToolRounds claimCount := by
  rw [adaptive_tool_rounds_le_individual_iff]
  omega

theorem batch_count_bound_alone_does_not_prove_round_savings :
    ∃ claimCount batchCount unresolvedCount,
      0 < claimCount ∧
      batchCount ≤ claimCount ∧
      individualToolRounds claimCount < adaptiveToolRounds batchCount unresolvedCount := by
  exact ⟨2, 1, 2, by decide, by decide, by decide⟩

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

theorem semantic_and_agent_failures_are_not_fallback_eligible :
    fallbackEligibleReason .semanticRejection = false ∧
    fallbackEligibleReason .policyMismatch = false ∧
    fallbackEligibleReason .replayMismatch = false ∧
    fallbackEligibleReason .agentPreference = false := by
  decide

end ASPProof.AXLE.SearchRouteAdaptiveBatchFailureIsolationCost
