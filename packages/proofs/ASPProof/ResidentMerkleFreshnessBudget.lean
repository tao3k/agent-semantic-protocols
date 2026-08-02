namespace ASPProof.ResidentMerkleFreshnessBudget

structure Observation where
  epoch : Nat
  root : Nat
  changedPathDigest : Nat
  deriving DecidableEq, Repr

structure PublishedGeneration where
  observationEpoch : Nat
  root : Nat
  selectorSetDigest : Nat
  deriving DecidableEq, Repr

inductive ForegroundResult where
  | fresh
  | generationStale
  | generationBuilding
  deriving DecidableEq, Repr

def covers (generation : PublishedGeneration) (observation : Observation) : Prop :=
  generation.observationEpoch = observation.epoch ∧ generation.root = observation.root

def foregroundAllowed
    (generation : PublishedGeneration)
    (observation : Observation)
    (result : ForegroundResult) : Prop :=
  match result with
  | .fresh => covers generation observation
  | .generationStale | .generationBuilding => True

def publicationAllowed
    (workerObservation latestObservation : Observation) : Prop :=
  workerObservation = latestObservation

structure PhaseBudgets where
  enqueueMicros : Nat
  acceptanceMicros : Nat
  warmReadMicros : Nat
  publicationMicros : Nat
  deriving DecidableEq, Repr

structure PhaseElapsed where
  enqueueMicros : Nat
  acceptanceMicros : Nat
  warmReadMicros : Nat
  publicationMicros : Nat
  deriving DecidableEq, Repr

def withinBudgets (budget : PhaseBudgets) (elapsed : PhaseElapsed) : Prop :=
  elapsed.enqueueMicros ≤ budget.enqueueMicros ∧
  elapsed.acceptanceMicros ≤ budget.acceptanceMicros ∧
  elapsed.warmReadMicros ≤ budget.warmReadMicros ∧
  elapsed.publicationMicros ≤ budget.publicationMicros

theorem stale_miss_cannot_be_fresh
    (generation : PublishedGeneration)
    (observation : Observation)
    (h : ¬ covers generation observation) :
    ¬ foregroundAllowed generation observation .fresh := by
  simpa [foregroundAllowed] using h

theorem obsolete_worker_cannot_publish
    (workerObservation latestObservation : Observation)
    (h : workerObservation ≠ latestObservation) :
    ¬ publicationAllowed workerObservation latestObservation := by
  simpa [publicationAllowed] using h

theorem phase_gates_are_conjunctive
    (budget : PhaseBudgets)
    (elapsed : PhaseElapsed)
    (h : withinBudgets budget elapsed) :
    elapsed.enqueueMicros ≤ budget.enqueueMicros ∧
      elapsed.acceptanceMicros ≤ budget.acceptanceMicros ∧
      elapsed.warmReadMicros ≤ budget.warmReadMicros ∧
      elapsed.publicationMicros ≤ budget.publicationMicros := by
  exact h

structure ServerProcess where
  id : Nat
  deriving DecidableEq, Repr

structure WorkspaceResident where
  workspaceIdentity : Nat
  owner : ServerProcess
  deriving DecidableEq, Repr

structure AgentSession where
  id : Nat
  server : ServerProcess
  deriving DecidableEq, Repr

def residentOwnedBy
    (resident : WorkspaceResident)
    (server : ServerProcess) : Prop :=
  resident.owner = server

theorem workspace_resident_owner_is_server
    (resident : WorkspaceResident) :
    residentOwnedBy resident resident.owner := by
  rfl

inductive MutationOwnership where
  | clientLocal
  | residentAccepted
  deriving DecidableEq, Repr

def survivesClientExit : MutationOwnership → Prop
  | .clientLocal => False
  | .residentAccepted => True

theorem queued_requires_resident_acceptance
    (ownership : MutationOwnership)
    (h : survivesClientExit ownership) :
    ownership = .residentAccepted := by
  cases ownership <;> simp [survivesClientExit] at h ⊢

end ASPProof.ResidentMerkleFreshnessBudget
