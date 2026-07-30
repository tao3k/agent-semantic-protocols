import ASPProof.SearchLoopClauseFirst
import Std.Tactic

namespace SearchLoopTrace

open SearchLoopClauseFirst

inductive LoopStatus where
  | running
  | closed
  | needsBudget
  deriving DecidableEq, Repr

structure LoopState where
  domain : ValidationDomain
  remainingMandatory : Nat
  unresolvedWork : Nat
  budget : Nat
  evidence : ClosureEvidence
  status : LoopStatus
  deriving DecidableEq, Repr

def remainingWork (state : LoopState) : Nat :=
  state.remainingMandatory + state.unresolvedWork

def SafeToClose (state : LoopState) : Prop :=
  state.remainingMandatory = 0 ∧ closed state.evidence

def ClosureSafe (state : LoopState) : Prop :=
  state.status = LoopStatus.closed → SafeToClose state

def TypedTerminal (state : LoopState) : Prop :=
  state.status = LoopStatus.closed ∨ state.status = LoopStatus.needsBudget

inductive Step : LoopState → LoopState → Prop where
  | discharge
      (domain : ValidationDomain)
      (evidence : ClosureEvidence)
      (remaining budget unresolved : Nat) :
      Step
        { domain := domain
          remainingMandatory := remaining + 1
          unresolvedWork := unresolved
          budget := budget + 1
          evidence := evidence
          status := LoopStatus.running }
        { domain := domain
          remainingMandatory := remaining
          unresolvedWork := unresolved
          budget := budget
          evidence := evidence
          status := LoopStatus.running }
  | resolve
      (domain : ValidationDomain)
      (evidence : ClosureEvidence)
      (remaining budget unresolved : Nat) :
      Step
        { domain := domain
          remainingMandatory := remaining
          unresolvedWork := unresolved + 1
          budget := budget + 1
          evidence := evidence
          status := LoopStatus.running }
        { domain := domain
          remainingMandatory := remaining
          unresolvedWork := unresolved
          budget := budget
          evidence := evidence
          status := LoopStatus.running }
  | close
      (state : LoopState)
      (running : state.status = LoopStatus.running)
      (safe : SafeToClose state) :
      Step state { state with status := LoopStatus.closed }
  | needsBudget
      (state : LoopState)
      (running : state.status = LoopStatus.running)
      (insufficient : state.budget < remainingWork state) :
      Step state { state with status := LoopStatus.needsBudget }

inductive Trace : LoopState → LoopState → Nat → Prop where
  | refl (state : LoopState) : Trace state state 0
  | tail {initial middle final : LoopState} {length : Nat} :
      Trace initial middle length →
      Step middle final →
      Trace initial final (length + 1)

inductive ProductiveStep : LoopState → LoopState → Prop where
  | discharge
      (domain : ValidationDomain)
      (evidence : ClosureEvidence)
      (remaining budget unresolved : Nat) :
      ProductiveStep
        { domain := domain
          remainingMandatory := remaining + 1
          unresolvedWork := unresolved
          budget := budget + 1
          evidence := evidence
          status := LoopStatus.running }
        { domain := domain
          remainingMandatory := remaining
          unresolvedWork := unresolved
          budget := budget
          evidence := evidence
          status := LoopStatus.running }
  | resolve
      (domain : ValidationDomain)
      (evidence : ClosureEvidence)
      (remaining budget unresolved : Nat) :
      ProductiveStep
        { domain := domain
          remainingMandatory := remaining
          unresolvedWork := unresolved + 1
          budget := budget + 1
          evidence := evidence
          status := LoopStatus.running }
        { domain := domain
          remainingMandatory := remaining
          unresolvedWork := unresolved
          budget := budget
          evidence := evidence
          status := LoopStatus.running }

inductive ProductiveTrace : LoopState → LoopState → Nat → Prop where
  | refl (state : LoopState) : ProductiveTrace state state 0
  | tail {initial middle final : LoopState} {length : Nat} :
      ProductiveTrace initial middle length →
      ProductiveStep middle final →
      ProductiveTrace initial final (length + 1)

theorem step_preserves_domain {before after : LoopState}
    (step : Step before after) :
    after.domain = before.domain := by
  cases step <;> rfl

theorem trace_preserves_domain {initial final : LoopState} {length : Nat}
    (trace : Trace initial final length) :
    final.domain = initial.domain := by
  induction trace with
  | refl => rfl
  | tail tracePrefix step inductionHypothesis =>
      exact (step_preserves_domain step).trans inductionHypothesis

theorem step_preserves_closure_safety {before after : LoopState}
    (_beforeSafe : ClosureSafe before)
    (step : Step before after) :
    ClosureSafe after := by
  intro afterClosed
  cases step with
  | discharge => cases afterClosed
  | resolve => cases afterClosed
  | close state running safe =>
      simpa [SafeToClose] using safe
  | needsBudget => cases afterClosed

theorem trace_preserves_closure_safety
    {initial final : LoopState} {length : Nat}
    (initialSafe : ClosureSafe initial)
    (trace : Trace initial final length) :
    ClosureSafe final := by
  induction trace with
  | refl => exact initialSafe
  | tail tracePrefix step inductionHypothesis =>
      exact step_preserves_closure_safety inductionHypothesis step

theorem productive_step_exactly_reduces_work
    {before after : LoopState}
    (step : ProductiveStep before after) :
    remainingWork after + 1 = remainingWork before := by
  cases step <;> simp [remainingWork, Nat.add_assoc, Nat.add_comm,
    Nat.add_left_comm]

theorem productive_step_decreases_work
    {before after : LoopState}
    (step : ProductiveStep before after) :
    remainingWork after < remainingWork before := by
  have exactDecrease := productive_step_exactly_reduces_work step
  rw [← exactDecrease]
  exact Nat.lt_succ_self _

theorem productive_trace_conserves_work
    {initial final : LoopState} {length : Nat}
    (trace : ProductiveTrace initial final length) :
    length + remainingWork final = remainingWork initial := by
  induction trace with
  | refl => simp
  | tail tracePrefix step inductionHypothesis =>
      cases step <;>
        simp_all [remainingWork, Nat.add_assoc, Nat.add_comm, Nat.add_left_comm]

theorem productive_trace_is_bounded
    {initial final : LoopState} {length : Nat}
    (trace : ProductiveTrace initial final length) :
    length ≤ remainingWork initial := by
  have conservation := productive_trace_conserves_work trace
  calc
    length ≤ length + remainingWork final := Nat.le_add_right _ _
    _ = remainingWork initial := conservation

theorem step_decreases_work_or_typed_terminal
    {before after : LoopState}
    (step : Step before after) :
    remainingWork after < remainingWork before ∨ TypedTerminal after := by
  cases step with
  | discharge =>
      left
      simp [remainingWork]
  | resolve =>
      left
      simp [remainingWork]
  | close =>
      right
      exact Or.inl rfl
  | needsBudget =>
      right
      exact Or.inr rfl

def invalidClosureEvidence : ClosureEvidence :=
  { allRequiredDischarged := true
    contradictionFree := true
    fresh := false
    exactMaterializationSatisfied := true }

def zeroMandatoryInvalidEvidenceState : LoopState :=
  { domain := exampleDomain
    remainingMandatory := 0
    unresolvedWork := 0
    budget := 0
    evidence := invalidClosureEvidence
    status := LoopStatus.running }

theorem zero_mandatory_does_not_imply_safe_closure :
    zeroMandatoryInvalidEvidenceState.remainingMandatory = 0 ∧
      ¬SafeToClose zeroMandatoryInvalidEvidenceState := by
  simp [zeroMandatoryInvalidEvidenceState, SafeToClose, invalidClosureEvidence,
    SearchLoopClauseFirst.closed]

def mandatoryOnlyBudgetState : LoopState :=
  { domain := exampleDomain
    remainingMandatory := 1
    unresolvedWork := 1
    budget := 1
    evidence := invalidClosureEvidence
    status := LoopStatus.running }

theorem mandatory_only_budget_undercounts_unresolved_work :
    mandatoryOnlyBudgetState.remainingMandatory ≤ mandatoryOnlyBudgetState.budget ∧
      mandatoryOnlyBudgetState.budget < remainingWork mandatoryOnlyBudgetState := by
  decide

def blockedState : LoopState :=
  { domain := exampleDomain
    remainingMandatory := 1
    unresolvedWork := 0
    budget := 0
    evidence := invalidClosureEvidence
    status := LoopStatus.running }

def blockedTerminal : LoopState :=
  { blockedState with status := LoopStatus.needsBudget }

theorem typed_terminalization_need_not_decrease_work :
    Step blockedState blockedTerminal ∧
      remainingWork blockedTerminal = remainingWork blockedState := by
  constructor
  · exact Step.needsBudget blockedState rfl (by decide)
  · rfl

end SearchLoopTrace
