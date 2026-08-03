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

end ASPProof.RuntimeServerGenerationReadiness
