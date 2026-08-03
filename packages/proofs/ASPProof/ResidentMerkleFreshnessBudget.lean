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

inductive AdmissionState where
  | building
  | ready
  | failed
  deriving DecidableEq, Repr

def ensureState : AdmissionState → AdmissionState
  | .failed => .building
  | state => state

theorem failed_admission_is_retryable :
    ensureState .failed = .building := by
  rfl

inductive ExactProjectionKind where
  | source
  | callableSkeleton
  deriving DecidableEq, Repr

structure ExactOwnerReadEvidence where
  cachedHit : Bool
  liveContentDigest : Nat
  publishedContentDigest : Nat
  deriving DecidableEq, Repr

def ownerFresh (evidence : ExactOwnerReadEvidence) : Prop :=
  evidence.liveContentDigest = evidence.publishedContentDigest

def exactProjectionAllowed
    (_kind : ExactProjectionKind)
    (evidence : ExactOwnerReadEvidence) : Prop :=
  ownerFresh evidence

theorem cached_hit_without_owner_freshness_is_insufficient :
    ∃ evidence : ExactOwnerReadEvidence,
      evidence.cachedHit = true ∧ ¬ ownerFresh evidence := by
  refine ⟨
    { cachedHit := true, liveContentDigest := 2, publishedContentDigest := 1 },
    rfl,
    ?_
  ⟩
  simp [ownerFresh]

theorem every_exact_projection_requires_owner_freshness
    (kind : ExactProjectionKind)
    (evidence : ExactOwnerReadEvidence)
    (h : exactProjectionAllowed kind evidence) :
    ownerFresh evidence := by
  exact h

theorem stale_cached_hit_cannot_be_returned
    (kind : ExactProjectionKind)
    (evidence : ExactOwnerReadEvidence)
    (hStale : evidence.liveContentDigest ≠ evidence.publishedContentDigest) :
    ¬ exactProjectionAllowed kind evidence := by
  simpa [exactProjectionAllowed, ownerFresh] using hStale

end ASPProof.ResidentMerkleFreshnessBudget
