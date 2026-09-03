namespace ASPProof.RuntimeServerGenerationReadiness

inductive LocatorState where
  | ready
  | missing
  | recoveryRequired
  deriving DecidableEq

inductive Operation where
  | acquireResidentLease
  | discoverCheckoutCandidate
  | admitGeneration
  deriving DecidableEq

inductive AdmissionMode where
  | completeGeneration
  | fullRecovery
  deriving DecidableEq

inductive ReadinessRequestState where
  | accepted
  | coalesced
  | ready
  deriving DecidableEq

def isCompleteReadBarrier (mode : AdmissionMode) (hasCommit : Bool) : Bool :=
  (mode == .completeGeneration || mode == .fullRecovery) && hasCommit

def requestReadiness
    (observedMode : Option AdmissionMode)
    (observedCommit : Bool)
    (completeRequestPending : Bool) : ReadinessRequestState :=
  if observedMode == some .fullRecovery && observedCommit then
    .ready
  else if completeRequestPending then
    .coalesced
  else
    .accepted

def readinessPublicationMode (_providerHint : Option Nat) : AdmissionMode :=
  .completeGeneration

def admissionTargetValid
    (mode : AdmissionMode)
    (providerTarget : Option Nat) : Bool :=
  match mode with
  | .completeGeneration => providerTarget.isNone
  | .fullRecovery => providerTarget.isNone

def readinessPlan : LocatorState -> List Operation
  | .ready => [.acquireResidentLease]
  | .missing => [.admitGeneration, .acquireResidentLease]
  | .recoveryRequired => [.admitGeneration, .acquireResidentLease]

def mayEnterQuery (state : LocatorState) (receiptValid : Bool) : Bool :=
  state == .ready && receiptValid

theorem ready_plan_is_checkout_free :
    Operation.discoverCheckoutCandidate ∉ readinessPlan .ready := by
  simp [readinessPlan]

theorem ready_plan_is_build_free :
    Operation.admitGeneration ∉ readinessPlan .ready := by
  simp [readinessPlan]

theorem unreadable_locator_cannot_enter_query
    (state : LocatorState)
    (h : state ≠ .ready) :
    mayEnterQuery state true = false := by
  cases state <;> simp_all [mayEnterQuery]

theorem recovery_requires_admission
    (state : LocatorState)
    (h : state = .missing ∨ state = .recoveryRequired) :
    Operation.admitGeneration ∈ readinessPlan state := by
  rcases h with rfl | rfl <;> simp [readinessPlan]

theorem complete_generation_commit_is_a_read_barrier :
    isCompleteReadBarrier .completeGeneration true = true := by
  rfl

theorem incomplete_generation_without_pending_observer_accepts_readiness :
    requestReadiness (some .completeGeneration) false false = .accepted := by
  rfl

theorem incomplete_generation_with_pending_observer_coalesces_duplicate :
    requestReadiness (some .completeGeneration) false true = .coalesced := by
  rfl

theorem complete_commit_is_the_unique_ready_observation :
    requestReadiness (some .fullRecovery) true false = .ready := by
  rfl

theorem provider_hint_cannot_create_targeted_publication
    (providerHint : Option Nat) :
    readinessPublicationMode providerHint = .completeGeneration := by
  rfl

theorem complete_generation_has_no_provider_target :
    admissionTargetValid .completeGeneration none = true := by
  rfl

theorem full_recovery_has_no_provider_target :
    admissionTargetValid .fullRecovery none = true := by
  rfl

end ASPProof.RuntimeServerGenerationReadiness
