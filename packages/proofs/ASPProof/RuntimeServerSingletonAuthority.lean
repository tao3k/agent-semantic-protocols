import ASPProof.SearchRouterInteractiveGraphState

namespace ASPProof

inductive RuntimeStateHomeKind where
  | installed
  | isolated
  deriving DecidableEq

structure RuntimeServerOwnerCandidate where
  stateHomeKind : RuntimeStateHomeKind
  isCanonicalExecutable : Bool

def RuntimeServerOwnerCandidate.mayAcquire
    (candidate : RuntimeServerOwnerCandidate) : Prop :=
  candidate.stateHomeKind = .isolated ∨ candidate.isCanonicalExecutable = true

theorem debugCannotAcquireInstalledStateHome :
    ¬ RuntimeServerOwnerCandidate.mayAcquire ⟨.installed, false⟩ := by
  simp [RuntimeServerOwnerCandidate.mayAcquire]

theorem debugMayAcquireIsolatedStateHome :
    RuntimeServerOwnerCandidate.mayAcquire ⟨.isolated, false⟩ := by
  simp [RuntimeServerOwnerCandidate.mayAcquire]

structure ResidentAdoptionEvidence where
  listenerBound : Bool
  endpointEpochMatches : Bool
  bindingTokenMatches : Bool

def ResidentAdoptionEvidence.adoptable (evidence : ResidentAdoptionEvidence) : Prop :=
  evidence.listenerBound = true ∧
    evidence.endpointEpochMatches = true ∧
    evidence.bindingTokenMatches = true

theorem listenerAloneDoesNotProveAdoption :
    ¬ ResidentAdoptionEvidence.adoptable ⟨true, false, false⟩ := by
  simp [ResidentAdoptionEvidence.adoptable]

inductive RuntimeDrainCause where
  | processShutdown
  | workspaceGenerationEvent
  deriving DecidableEq

def mayGloballyDrain : RuntimeDrainCause → Bool
  | .processShutdown => true
  | .workspaceGenerationEvent => false

theorem workspaceGenerationCannotGloballyDrain :
    mayGloballyDrain .workspaceGenerationEvent = false := by
  rfl

structure ResidentLifecycleIo where
  tokioOwned : Bool
  deadlineBounded : Bool
  cancellationAware : Bool

def ResidentLifecycleIo.admissible (step : ResidentLifecycleIo) : Prop :=
  step.tokioOwned = true ∧
    step.deadlineBounded = true ∧
    step.cancellationAware = true

theorem synchronousCoreWorkerIoIsRejected :
    ¬ ResidentLifecycleIo.admissible ⟨false, false, false⟩ := by
  simp [ResidentLifecycleIo.admissible]

inductive SingletonProbeOutcome where
  | live
  | stale
  | timedOut
  | cancelled
  deriving DecidableEq

def mayUnlinkSingleton : SingletonProbeOutcome → Bool
  | .stale => true
  | .live | .timedOut | .cancelled => false

theorem timedOutProbeFailsClosed :
    mayUnlinkSingleton .timedOut = false := by
  rfl

theorem cancelledProbeFailsClosed :
    mayUnlinkSingleton .cancelled = false := by
  rfl

structure SingletonRelease where
  awaited : Bool
  inodeMatches : Bool

def SingletonRelease.mayRemove (release : SingletonRelease) : Prop :=
  release.awaited = true ∧ release.inodeMatches = true

theorem dropFallbackCannotClaimNormalRelease :
    ¬ SingletonRelease.mayRemove ⟨false, true⟩ := by
  simp [SingletonRelease.mayRemove]

structure FallbackReleaseTransfer where
  ownershipTransferred : Bool
  futurePolled : Bool

def FallbackReleaseTransfer.mayReschedule
    (transfer : FallbackReleaseTransfer) : Bool :=
  !transfer.ownershipTransferred && !transfer.futurePolled

theorem cancelledTransferredReleaseCannotReschedule :
    FallbackReleaseTransfer.mayReschedule ⟨true, false⟩ = false := by
  rfl

end ASPProof
