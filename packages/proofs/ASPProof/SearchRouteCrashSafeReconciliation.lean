namespace ASPProof.SearchRouteCrashSafeReconciliation

inductive DurablePhase where
  | admitted
  | executing
  | completed
  deriving DecidableEq, Repr

inductive RemoteStatus where
  | notStarted
  | completed
  | unknown
  deriving DecidableEq, Repr

inductive ResumeAction where
  | execute
  | reconcile
  | acceptCompleted
  | stayCompleted
  deriving DecidableEq, Repr

structure DurableCheckpoint where
  phase : DurablePhase
  requestDigest : Nat
  effectNonce : Nat
  contextDigest : Nat
  deriving DecidableEq, Repr

structure RemoteObservation where
  requestDigest : Nat
  effectNonce : Nat
  status : RemoteStatus
  deriving DecidableEq, Repr

def ObservationBound
    (checkpoint : DurableCheckpoint)
    (observation : RemoteObservation) : Prop :=
  checkpoint.requestDigest = observation.requestDigest ∧
  checkpoint.effectNonce = observation.effectNonce

def SafeResume
    (checkpoint : DurableCheckpoint)
    (observation : RemoteObservation)
    (action : ResumeAction) : Prop :=
  ObservationBound checkpoint observation ∧
  match checkpoint.phase, observation.status, action with
  | .admitted, .notStarted, .execute => True
  | .admitted, .unknown, .reconcile => True
  | .executing, .unknown, .reconcile => True
  | .executing, .completed, .acceptCompleted => True
  | .completed, _, .stayCompleted => True
  | _, _, _ => False

def ambiguousCheckpoint : DurableCheckpoint :=
  {
    phase := .executing
    requestDigest := 111
    effectNonce := 121
    contextDigest := 101
  }

def ambiguousObservation : RemoteObservation :=
  {
    requestDigest := 111
    effectNonce := 121
    status := .unknown
  }

theorem ambiguous_execution_permits_reconciliation :
    SafeResume
      ambiguousCheckpoint
      ambiguousObservation
      .reconcile := by
  decide

theorem ambiguous_execution_rejects_execute :
    ¬ SafeResume
      ambiguousCheckpoint
      ambiguousObservation
      .execute := by
  decide

theorem ambiguous_execution_rejects_accept_completed :
    ¬ SafeResume
      ambiguousCheckpoint
      ambiguousObservation
      .acceptCompleted := by
  decide

def mismatchedObservation : RemoteObservation :=
  { ambiguousObservation with requestDigest := 999 }

theorem identity_mismatch_rejects_every_action
    (action : ResumeAction) :
    ¬ SafeResume
      ambiguousCheckpoint
      mismatchedObservation
      action := by
  intro alleged
  exact (by decide) alleged.1.1

def admittedCheckpoint : DurableCheckpoint :=
  {
    phase := .admitted
    requestDigest := 112
    effectNonce := 122
    contextDigest := 101
  }

def notStartedObservation : RemoteObservation :=
  {
    requestDigest := 112
    effectNonce := 122
    status := .notStarted
  }

theorem admitted_not_started_permits_execute :
    SafeResume
      admittedCheckpoint
      notStartedObservation
      .execute := by
  decide

def completedCheckpoint : DurableCheckpoint :=
  {
    phase := .completed
    requestDigest := 113
    effectNonce := 123
    contextDigest := 101
  }

def completedObservation : RemoteObservation :=
  {
    requestDigest := 113
    effectNonce := 123
    status := .completed
  }

theorem completed_checkpoint_permits_stay_completed :
    SafeResume
      completedCheckpoint
      completedObservation
      .stayCompleted := by
  decide

theorem completed_checkpoint_rejects_execute :
    ¬ SafeResume
      completedCheckpoint
      completedObservation
      .execute := by
  decide

theorem completed_checkpoint_rejects_reconciliation :
    ¬ SafeResume
      completedCheckpoint
      completedObservation
      .reconcile := by
  decide

theorem completed_checkpoint_rejects_accept_completed :
    ¬ SafeResume
      completedCheckpoint
      completedObservation
      .acceptCompleted := by
  decide

end ASPProof.SearchRouteCrashSafeReconciliation
