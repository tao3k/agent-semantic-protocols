import ASPProof.RuntimeWorkspaceGeneration

namespace ASPProof.RuntimeSupervisorReconciliation

inductive Dependency where
  | endpointReceipt
  | canonicalArtifact
  | operatingSystemSupervisor
  | workspaceDataPlane
  deriving DecidableEq, Repr

def supervisorDependencies : List Dependency :=
  [.endpointReceipt, .canonicalArtifact, .operatingSystemSupervisor]

theorem supervisor_control_is_data_plane_independent :
    .workspaceDataPlane ∉ supervisorDependencies := by
  simp [supervisorDependencies]

structure EndpointObservation where
  authenticated : Bool
  runtimeArtifactCurrent : Bool
  transportContractCurrent : Bool
  deriving DecidableEq, Repr

def statusAdmitted (endpoint : EndpointObservation) : Bool :=
  endpoint.authenticated && endpoint.transportContractCurrent

def reconcileRequestsDrain (endpoint : EndpointObservation) : Bool :=
  endpoint.authenticated &&
    (!endpoint.runtimeArtifactCurrent || !endpoint.transportContractCurrent)

theorem stale_transport_rejects_status
    (runtimeCurrent : Bool) :
    statusAdmitted ⟨true, runtimeCurrent, false⟩ = false := by
  simp [statusAdmitted]

theorem stale_transport_requests_authenticated_drain
    (runtimeCurrent : Bool) :
    reconcileRequestsDrain ⟨true, runtimeCurrent, false⟩ = true := by
  simp [reconcileRequestsDrain]

theorem matching_runtime_and_transport_is_noop :
    reconcileRequestsDrain ⟨true, true, true⟩ = false := by
  decide

theorem unauthenticated_reconcile_cannot_drain
    (runtimeCurrent transportCurrent : Bool) :
    reconcileRequestsDrain ⟨false, runtimeCurrent, transportCurrent⟩ = false := by
  simp [reconcileRequestsDrain]

structure ReconcileGate where
  ownerEpoch : Option Nat
  deriving DecidableEq, Repr

def admit (gate : ReconcileGate) (candidateEpoch : Nat) : ReconcileGate × Bool :=
  match gate.ownerEpoch with
  | none => (⟨some candidateEpoch⟩, true)
  | some _ => (gate, false)

theorem first_reconcile_owns_single_flight (candidateEpoch : Nat) :
    (admit ⟨none⟩ candidateEpoch).2 = true := by
  simp [admit]

theorem concurrent_reconcile_does_not_create_second_owner
    (ownerEpoch candidateEpoch : Nat) :
    (admit ⟨some ownerEpoch⟩ candidateEpoch).2 = false := by
  simp [admit]

inductive Stage where
  | intent
  | draining
  | replacing
  | publishingHealthyReceipt
  | healthy
  deriving DecidableEq, Repr

def next : Stage → Stage
  | .intent => .draining
  | .draining => .replacing
  | .replacing => .publishingHealthyReceipt
  | .publishingHealthyReceipt => .healthy
  | .healthy => .healthy

def rank : Stage → Nat
  | .intent => 4
  | .draining => 3
  | .replacing => 2
  | .publishingHealthyReceipt => 1
  | .healthy => 0

theorem reconciliation_strictly_progresses
    (stage : Stage) (notHealthy : stage ≠ .healthy) :
    rank (next stage) < rank stage := by
  cases stage <;> simp [next, rank] at notHealthy ⊢

theorem bounded_reconciliation_reaches_healthy :
    next (next (next (next .intent))) = .healthy := by
  decide

inductive HookReceipt where
  | ready
  | reconcileScheduled
  deriving DecidableEq, Repr

def hookReceipt (endpoint : EndpointObservation) : HookReceipt :=
  if reconcileRequestsDrain endpoint then .reconcileScheduled else .ready

theorem hook_reconciliation_never_waits
    (endpoint : EndpointObservation) :
    hookReceipt endpoint = .ready ∨ hookReceipt endpoint = .reconcileScheduled := by
  simp only [hookReceipt]
  split <;> simp_all

end ASPProof.RuntimeSupervisorReconciliation
