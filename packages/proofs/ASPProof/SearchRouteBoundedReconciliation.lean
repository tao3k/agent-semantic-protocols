-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.SearchRouteBoundedReconciliation

inductive RecoveryPhase where
  | reconciling
  | completed
  | safeReleased
  | quarantined
  deriving DecidableEq, Repr

inductive LedgerDisposition where
  | held
  | consumed
  | released
  | quarantined
  deriving DecidableEq, Repr

structure RecoveryState where
  phase : RecoveryPhase
  ledger : LedgerDisposition
  attemptsRemaining : Nat
  roundsRemaining : Nat
  transitionsRemaining : Nat
  deriving DecidableEq, Repr

def ContinueStep
    (current next : RecoveryState) : Prop :=
  current.phase = .reconciling ∧
  current.ledger = .held ∧
  next.phase = .reconciling ∧
  next.ledger = .held ∧
  next.attemptsRemaining < current.attemptsRemaining ∧
  next.roundsRemaining < current.roundsRemaining ∧
  next.transitionsRemaining < current.transitionsRemaining

def Exhausted (state : RecoveryState) : Prop :=
  state.attemptsRemaining = 0 ∨
  state.roundsRemaining = 0 ∨
  state.transitionsRemaining = 0

def QuarantineResolution
    (current next : RecoveryState) : Prop :=
  current.phase = .reconciling ∧
  current.ledger = .held ∧
  Exhausted current ∧
  next.phase = .quarantined ∧
  next.ledger = .quarantined

theorem reconciliation_self_loop_is_rejected
    (state : RecoveryState) :
    ¬ ContinueStep state state := by
  intro step
  exact
    (Nat.lt_irrefl state.attemptsRemaining)
      step.2.2.2.2.1

theorem exhausted_state_has_no_continuation
    (current next : RecoveryState)
    (exhausted : Exhausted current) :
    ¬ ContinueStep current next := by
  intro step
  rcases exhausted with attemptsZero | roundsOrTransitionsZero
  · have belowZero : next.attemptsRemaining < 0 := by
      simpa [attemptsZero] using step.2.2.2.2.1
    exact (Nat.not_lt_zero next.attemptsRemaining) belowZero
  · rcases roundsOrTransitionsZero with roundsZero | transitionsZero
    · have belowZero : next.roundsRemaining < 0 := by
        simpa [roundsZero] using step.2.2.2.2.2.1
      exact (Nat.not_lt_zero next.roundsRemaining) belowZero
    · have belowZero : next.transitionsRemaining < 0 := by
        simpa [transitionsZero] using step.2.2.2.2.2.2
      exact (Nat.not_lt_zero next.transitionsRemaining) belowZero

inductive ReconciliationRun :
    RecoveryState → RecoveryState → Nat → Prop where
  | done (state : RecoveryState) :
      ReconciliationRun state state 0
  | step
      {current next final : RecoveryState}
      {steps : Nat}
      (progress : ContinueStep current next)
      (rest : ReconciliationRun next final steps) :
      ReconciliationRun current final (Nat.succ steps)

theorem run_length_le_initial_attempts
    {initial final : RecoveryState}
    {steps : Nat}
    (run : ReconciliationRun initial final steps) :
    steps ≤ initial.attemptsRemaining := by
  induction run with
  | done state =>
      exact Nat.zero_le state.attemptsRemaining
  | step progress rest inductionHypothesis =>
      exact Nat.le_trans
        (Nat.succ_le_succ inductionHypothesis)
        (Nat.succ_le_of_lt progress.2.2.2.2.1)

theorem run_length_le_initial_rounds
    {initial final : RecoveryState}
    {steps : Nat}
    (run : ReconciliationRun initial final steps) :
    steps ≤ initial.roundsRemaining := by
  induction run with
  | done state =>
      exact Nat.zero_le state.roundsRemaining
  | step progress rest inductionHypothesis =>
      exact Nat.le_trans
        (Nat.succ_le_succ inductionHypothesis)
        (Nat.succ_le_of_lt progress.2.2.2.2.2.1)

theorem run_length_le_initial_transitions
    {initial final : RecoveryState}
    {steps : Nat}
    (run : ReconciliationRun initial final steps) :
    steps ≤ initial.transitionsRemaining := by
  induction run with
  | done state =>
      exact Nat.zero_le state.transitionsRemaining
  | step progress rest inductionHypothesis =>
      exact Nat.le_trans
        (Nat.succ_le_succ inductionHypothesis)
        (Nat.succ_le_of_lt progress.2.2.2.2.2.2)

def exhaustedExample : RecoveryState :=
  {
    phase := .reconciling
    ledger := .held
    attemptsRemaining := 0
    roundsRemaining := 1
    transitionsRemaining := 1
  }

def quarantinedExample : RecoveryState :=
  {
    phase := .quarantined
    ledger := .quarantined
    attemptsRemaining := 0
    roundsRemaining := 1
    transitionsRemaining := 1
  }

def unsafeReleasedExample : RecoveryState :=
  {
    phase := .safeReleased
    ledger := .released
    attemptsRemaining := 0
    roundsRemaining := 1
    transitionsRemaining := 1
  }

theorem exhausted_example_is_quarantined :
    QuarantineResolution exhaustedExample quarantinedExample := by
  unfold QuarantineResolution Exhausted exhaustedExample quarantinedExample
  decide

theorem exhausted_example_cannot_release :
    ¬ QuarantineResolution exhaustedExample unsafeReleasedExample := by
  unfold QuarantineResolution Exhausted exhaustedExample unsafeReleasedExample
  decide

end ASPProof.SearchRouteBoundedReconciliation
