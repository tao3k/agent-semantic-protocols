import ASPProof.SearchRoutePerDimensionEffectResolution

namespace ASPProof.SearchRouteMonotonePartialResolution

open ASPProof.SearchRoutePerDimensionEffectResolution

def OutcomeRefines
    (before after : DimensionOutcome) : Prop :=
  match before, after with
  | .unknown, _ => True
  | .consumed, .consumed => True
  | .noEffect, .noEffect => True
  | _, _ => False

theorem consumed_refines_only_to_consumed
    (after : DimensionOutcome)
    (refines : OutcomeRefines .consumed after) :
    after = .consumed := by
  cases after <;> simp [OutcomeRefines] at refines ⊢

theorem no_effect_refines_only_to_no_effect
    (after : DimensionOutcome)
    (refines : OutcomeRefines .noEffect after) :
    after = .noEffect := by
  cases after <;> simp [OutcomeRefines] at refines ⊢

def VectorRefines
    (before after : EffectResolutionVector) : Prop :=
  OutcomeRefines before.tokens after.tokens ∧
  OutcomeRefines before.money after.money ∧
  OutcomeRefines before.providerQuota after.providerQuota

def outcomeResolved : DimensionOutcome → Nat
  | .unknown => 0
  | .consumed => 1
  | .noEffect => 1

def outcomeUnknown : DimensionOutcome → Nat
  | .unknown => 1
  | .consumed => 0
  | .noEffect => 0

def resolvedCount (vector : EffectResolutionVector) : Nat :=
  outcomeResolved vector.tokens +
  outcomeResolved vector.money +
  outcomeResolved vector.providerQuota

def unknownCount (vector : EffectResolutionVector) : Nat :=
  outcomeUnknown vector.tokens +
  outcomeUnknown vector.money +
  outcomeUnknown vector.providerQuota

theorem resolved_plus_unknown_equals_three
    (vector : EffectResolutionVector) :
    resolvedCount vector + unknownCount vector = 3 := by
  cases vector with
  | mk tokens money providerQuota =>
      cases tokens <;> cases money <;> cases providerQuota <;>
        decide

def StrictVectorRefines
    (before after : EffectResolutionVector) : Prop :=
  VectorRefines before after ∧
  resolvedCount before < resolvedCount after

theorem strict_refinement_is_not_reflexive
    (vector : EffectResolutionVector) :
    ¬ StrictVectorRefines vector vector := by
  intro strict
  exact (Nat.lt_irrefl (resolvedCount vector)) strict.2

theorem strict_refinement_decreases_unknown
    (before after : EffectResolutionVector)
    (strict : StrictVectorRefines before after) :
    unknownCount after < unknownCount before := by
  have beforePartition :=
    resolved_plus_unknown_equals_three before
  have afterPartition :=
    resolved_plus_unknown_equals_three after
  omega

structure ResolutionSnapshot where
  vector : EffectResolutionVector
  revision : Nat
  deriving DecidableEq, Repr

def ResolutionStep
    (before after : ResolutionSnapshot) : Prop :=
  StrictVectorRefines before.vector after.vector ∧
  after.revision = before.revision + 1

theorem resolution_step_advances_revision
    (before after : ResolutionSnapshot)
    (step : ResolutionStep before after) :
    before.revision < after.revision := by
  rw [step.2]
  exact Nat.lt_succ_self before.revision

theorem resolution_step_decreases_unknown
    (before after : ResolutionSnapshot)
    (step : ResolutionStep before after) :
    unknownCount after.vector < unknownCount before.vector :=
  strict_refinement_decreases_unknown
    before.vector
    after.vector
    step.1

inductive ResolutionRun :
    ResolutionSnapshot → ResolutionSnapshot → Nat → Prop where
  | done (state : ResolutionSnapshot) :
      ResolutionRun state state 0
  | step
      {current next final : ResolutionSnapshot}
      {steps : Nat}
      (progress : ResolutionStep current next)
      (rest : ResolutionRun next final steps) :
      ResolutionRun current final (Nat.succ steps)

theorem resolution_run_length_le_initial_unknown
    {initial final : ResolutionSnapshot}
    {steps : Nat}
    (run : ResolutionRun initial final steps) :
    steps ≤ unknownCount initial.vector := by
  induction run with
  | done state =>
      exact Nat.zero_le (unknownCount state.vector)
  | step progress rest inductionHypothesis =>
      exact Nat.le_trans
        (Nat.succ_le_succ inductionHypothesis)
        (Nat.succ_le_of_lt
          (resolution_step_decreases_unknown _ _ progress))

def partialSnapshot : ResolutionSnapshot :=
  {
    vector := mixedOutcome
    revision := 1
  }

def finalizedSnapshot : ResolutionSnapshot :=
  {
    vector := {
      tokens := .noEffect
      money := .consumed
      providerQuota := .consumed
    }
    revision := 2
  }

def conflictingSnapshot : ResolutionSnapshot :=
  {
    vector := {
      tokens := .noEffect
      money := .noEffect
      providerQuota := .consumed
    }
    revision := 2
  }

theorem example_token_finalization_is_valid :
    ResolutionStep partialSnapshot finalizedSnapshot := by
  decide

theorem example_terminal_money_rewrite_is_invalid :
    ¬ ResolutionStep partialSnapshot conflictingSnapshot := by
  decide

theorem example_unknown_count_decreases :
    unknownCount finalizedSnapshot.vector <
      unknownCount partialSnapshot.vector := by
  decide

end ASPProof.SearchRouteMonotonePartialResolution

