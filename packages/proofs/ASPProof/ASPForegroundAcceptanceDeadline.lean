namespace ASPProof.ForegroundAcceptanceDeadline

structure ForegroundTiming where
  enqueueMicros : Nat
  acceptanceMicros : Nat
  deriving DecidableEq, Repr

def elapsedMicros (timing : ForegroundTiming) : Nat :=
  timing.enqueueMicros + timing.acceptanceMicros

def withinAbsoluteDeadline (limitMicros : Nat) (timing : ForegroundTiming) : Prop :=
  elapsedMicros timing ≤ limitMicros

def receiverOnlyBounded (limitMicros : Nat) (timing : ForegroundTiming) : Prop :=
  timing.acceptanceMicros ≤ limitMicros

def saturatedQueueCounterexample : ForegroundTiming :=
  { enqueueMicros := 100001, acceptanceMicros := 0 }

theorem receiver_only_timeout_does_not_bound_foreground :
    receiverOnlyBounded 100000 saturatedQueueCounterexample ∧
      ¬ withinAbsoluteDeadline 100000 saturatedQueueCounterexample := by
  constructor
  · unfold receiverOnlyBounded saturatedQueueCounterexample
    decide
  · unfold withinAbsoluteDeadline elapsedMicros saturatedQueueCounterexample
    decide

theorem absolute_deadline_bounds_enqueue_and_acceptance
    (timing : ForegroundTiming)
    (h : withinAbsoluteDeadline 100000 timing) :
    timing.enqueueMicros ≤ 100000 ∧ timing.acceptanceMicros ≤ 100000 := by
  constructor
  · exact Nat.le_trans (Nat.le_add_right _ _) h
  · exact Nat.le_trans (Nat.le_add_left _ _) h

inductive EnqueueOutcome where
  | notAccepted
  | accepted
  deriving DecidableEq, Repr

def backgroundContinuationRequired
    (outcome : EnqueueOutcome)
    (writerContinues : Bool) : Prop :=
  outcome = .accepted → writerContinues = true

theorem accepted_work_survives_foreground_timeout
    (writerContinues : Bool)
    (h : backgroundContinuationRequired .accepted writerContinues) :
    writerContinues = true := by
  exact h rfl

theorem nonaccepted_enqueue_has_no_continuation_obligation
    (writerContinues : Bool) :
    backgroundContinuationRequired .notAccepted writerContinues := by
  intro h
  cases h

end ASPProof.ForegroundAcceptanceDeadline
