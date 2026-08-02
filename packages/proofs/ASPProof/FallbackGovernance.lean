namespace ASPProof.FallbackGovernance

inductive LaneKind where
  | proof
  | runtime
  deriving DecidableEq, Repr

structure Lane where
  name : String
  kind : LaneKind
  rank : Nat
  guarantees : String → Prop

structure FallbackAttempt where
  source : Lane
  target : Lane
  failureObserved : Prop
  authorized : Prop
  degradationDeclared : Prop
  semanticsEquivalent : Prop
  cacheSeparated : Prop
  budgetBefore : Nat
  budgetAfter : Nat

def RankStep (target source : Lane) : Prop :=
  target.rank < source.rank

def RuntimeGuarded (attempt : FallbackAttempt) : Prop :=
  attempt.source.kind = .runtime ∧
    attempt.target.kind = .runtime ∧
    attempt.failureObserved ∧
    attempt.authorized ∧
    attempt.degradationDeclared ∧
    RankStep attempt.target attempt.source ∧
    attempt.budgetAfter < attempt.budgetBefore ∧
    (∀ guarantee, attempt.target.guarantees guarantee →
      attempt.source.guarantees guarantee) ∧
    (attempt.semanticsEquivalent ∨ attempt.cacheSeparated)

def SafeFor
    (required : String → Prop)
    (attempt : FallbackAttempt) : Prop :=
  RuntimeGuarded attempt ∧
    ∀ guarantee, required guarantee → attempt.target.guarantees guarantee

def OperationalFallbackAllowed (attempt : FallbackAttempt) : Prop :=
  attempt.source.kind = .runtime ∧ RuntimeGuarded attempt

theorem proof_lane_rejects_operational_fallback
    (attempt : FallbackAttempt)
    (hProof : attempt.source.kind = .proof) :
    ¬ OperationalFallbackAllowed attempt := by
  intro hAllowed
  rcases hAllowed with ⟨hRuntime, _⟩
  have hImpossible : LaneKind.proof = LaneKind.runtime :=
    hProof.symm.trans hRuntime
  exact LaneKind.noConfusion hImpossible

theorem no_eager_fallback
    (attempt : FallbackAttempt)
    (hPrimaryAvailable : ¬ attempt.failureObserved) :
    ¬ RuntimeGuarded attempt := by
  intro hGuarded
  rcases hGuarded with ⟨_, _, hFailure, _, _, _, _, _, _⟩
  exact hPrimaryAvailable hFailure

theorem no_unauthorized_fallback
    (attempt : FallbackAttempt)
    (hUnauthorized : ¬ attempt.authorized) :
    ¬ RuntimeGuarded attempt := by
  intro hGuarded
  rcases hGuarded with ⟨_, _, _, hAuthorized, _, _, _, _, _⟩
  exact hUnauthorized hAuthorized

theorem no_silent_degradation
    (attempt : FallbackAttempt)
    (hSilent : ¬ attempt.degradationDeclared) :
    ¬ RuntimeGuarded attempt := by
  intro hGuarded
  rcases hGuarded with ⟨_, _, _, _, hDeclared, _, _, _, _⟩
  exact hSilent hDeclared

theorem guarded_fallback_does_not_inflate_guarantees
    (attempt : FallbackAttempt)
    (hGuarded : RuntimeGuarded attempt)
    (guarantee : String)
    (hTarget : attempt.target.guarantees guarantee) :
    attempt.source.guarantees guarantee := by
  rcases hGuarded with ⟨_, _, _, _, _, _, _, hPreserves, _⟩
  exact hPreserves guarantee hTarget

theorem non_equivalent_fallback_separates_cache
    (attempt : FallbackAttempt)
    (hGuarded : RuntimeGuarded attempt)
    (hDifferent : ¬ attempt.semanticsEquivalent) :
    attempt.cacheSeparated := by
  rcases hGuarded with ⟨_, _, _, _, _, _, _, _, hCache⟩
  rcases hCache with hEquivalent | hSeparated
  · exact False.elim (hDifferent hEquivalent)
  · exact hSeparated

theorem missing_required_guarantee_denies_acceptance
    (required : String → Prop)
    (attempt : FallbackAttempt)
    (hMissing : ∃ guarantee,
      required guarantee ∧ ¬ attempt.target.guarantees guarantee) :
    ¬ SafeFor required attempt := by
  intro hSafe
  rcases hMissing with ⟨guarantee, hRequired, hAbsent⟩
  exact hAbsent (hSafe.2 guarantee hRequired)

theorem no_zero_budget_fallback
    (attempt : FallbackAttempt)
    (hZero : attempt.budgetBefore = 0) :
    ¬ RuntimeGuarded attempt := by
  intro hGuarded
  rcases hGuarded with ⟨_, _, _, _, _, _, hConsumes, _, _⟩
  rw [hZero] at hConsumes
  exact Nat.not_lt_zero attempt.budgetAfter hConsumes

theorem no_rank_cycle
    (first second : Lane)
    (hForward : RankStep second first) :
    ¬ RankStep first second := by
  intro hBackward
  exact (Nat.not_lt_of_ge (Nat.le_of_lt hForward)) hBackward

theorem rank_step_well_founded : WellFounded RankStep := by
  exact InvImage.wf Lane.rank Nat.lt_wfRel.wf

end ASPProof.FallbackGovernance
