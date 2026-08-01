namespace ASPProof.AgentSessionLifecycleSpawnObservationGap

structure CallerObservation where
  startedActivity : Bool
  toolResult : Bool
  deriving DecidableEq, Repr

structure HostState where
  childExists : Bool
  durableIntent : Bool
  intentIndexedAccept : Bool
  caller : CallerObservation
  deriving DecidableEq, Repr

def beforeSpawn : HostState :=
  ⟨false, false, false, ⟨false, false⟩⟩

def afterHostReturnBeforeActivity : HostState :=
  ⟨true, false, false, ⟨false, false⟩⟩

def afterActivityBeforeResult : HostState :=
  ⟨true, false, false, ⟨true, false⟩⟩

def callerProjection (state : HostState) : CallerObservation := state.caller

theorem preSpawnAndGapHaveSameCallerProjection :
    callerProjection beforeSpawn =
      callerProjection afterHostReturnBeforeActivity := by
  rfl

theorem preSpawnAndGapHaveDifferentHostOwnership :
    beforeSpawn.childExists ≠ afterHostReturnBeforeActivity.childExists := by
  decide

theorem callerObservationDoesNotDetermineHostOwnership :
    ∃ left right : HostState,
      callerProjection left = callerProjection right ∧
      left.childExists ≠ right.childExists := by
  exact ⟨beforeSpawn, afterHostReturnBeforeActivity,
    preSpawnAndGapHaveSameCallerProjection,
    preSpawnAndGapHaveDifferentHostOwnership⟩

def BindingAdmissible (state : HostState) : Prop :=
  state.childExists = true ∧
  state.durableIntent = true ∧
  state.intentIndexedAccept = true

theorem postSpawnActivityAloneDoesNotAdmitBinding :
    ¬ BindingAdmissible afterActivityBeforeResult := by
  intro admitted
  exact Bool.noConfusion admitted.2.1

structure ResidentSpawnRequest where
  canonicalTaskName : Bool
  typedRoleSelected : Bool
  forkTurnsNone : Bool
  deriving DecidableEq, Repr

def ResidentSpawnAdmissible (request : ResidentSpawnRequest) : Prop :=
  request.canonicalTaskName = true ∧
  request.typedRoleSelected = true ∧
  request.forkTurnsNone = true

def pathOnlyRequest : ResidentSpawnRequest :=
  ⟨true, false, true⟩

def defaultFullHistoryRequest : ResidentSpawnRequest :=
  ⟨true, true, false⟩

theorem canonicalTaskNameDoesNotProveTypedRole :
    ¬ ResidentSpawnAdmissible pathOnlyRequest := by
  intro admitted
  exact Bool.noConfusion admitted.2.1

theorem defaultFullHistoryDoesNotAdmitConfiguredResident :
    ¬ ResidentSpawnAdmissible defaultFullHistoryRequest := by
  intro admitted
  exact Bool.noConfusion admitted.2.2

structure DurableSpawnChain where
  reservationCommitted : Bool
  intentDurable : Bool
  hostAcceptIndexed : Bool
  startedReceiptIndexed : Bool
  bindingCommitted : Bool
  deriving DecidableEq, Repr

def ChainClosed (chain : DurableSpawnChain) : Prop :=
  chain.reservationCommitted = true ∧
  chain.intentDurable = true ∧
  chain.hostAcceptIndexed = true ∧
  chain.startedReceiptIndexed = true ∧
  chain.bindingCommitted = true

theorem closedChainHasDurablePreSpawnAuthority
    (chain : DurableSpawnChain) (closed : ChainClosed chain) :
    chain.reservationCommitted = true ∧ chain.intentDurable = true :=
  ⟨closed.1, closed.2.1⟩

theorem closedChainHasIndexedPostSpawnEvidence
    (chain : DurableSpawnChain) (closed : ChainClosed chain) :
    chain.hostAcceptIndexed = true ∧ chain.startedReceiptIndexed = true :=
  ⟨closed.2.2.1, closed.2.2.2.1⟩

structure RuntimeArtifactAdmission where
  digestValid : Bool
  executableClosureReady : Bool
  semanticResponseAccepted : Bool
  deriving DecidableEq, Repr

def DigestAdmitted (artifact : RuntimeArtifactAdmission) : Prop :=
  artifact.digestValid = true

def RuntimeAccepted (artifact : RuntimeArtifactAdmission) : Prop :=
  artifact.digestValid = true ∧
  artifact.executableClosureReady = true ∧
  artifact.semanticResponseAccepted = true

def digestValidWrapperWithoutClosure : RuntimeArtifactAdmission :=
  ⟨true, false, false⟩

def executableArtifactWithSemanticDrift : RuntimeArtifactAdmission :=
  ⟨true, true, false⟩

theorem digestIdentityDoesNotProveExecutableClosure :
    DigestAdmitted digestValidWrapperWithoutClosure ∧
    ¬ RuntimeAccepted digestValidWrapperWithoutClosure := by
  constructor
  · rfl
  · intro accepted
    exact Bool.noConfusion accepted.2.1

theorem executableClosureDoesNotProveSemanticAdmission :
    executableArtifactWithSemanticDrift.executableClosureReady = true ∧
    ¬ RuntimeAccepted executableArtifactWithSemanticDrift := by
  constructor
  · rfl
  · intro accepted
    exact Bool.noConfusion accepted.2.2

inductive ProviderRoute where
  | rust
  | typescript
  deriving DecidableEq, Repr

structure ProviderScopedGeneration where
  rustReady : Bool
  typescriptReady : Bool
  deriving DecidableEq, Repr

def RouteReady
    (generation : ProviderScopedGeneration) (route : ProviderRoute) : Prop :=
  match route with
  | .rust => generation.rustReady = true
  | .typescript => generation.typescriptReady = true

def GlobalConjunctionReady (generation : ProviderScopedGeneration) : Prop :=
  generation.rustReady = true ∧ generation.typescriptReady = true

def rustReadyTypescriptRejected : ProviderScopedGeneration :=
  ⟨true, false⟩

theorem unrelatedProviderFailurePreservesReadyRoute :
    RouteReady rustReadyTypescriptRejected .rust ∧
    ¬ RouteReady rustReadyTypescriptRejected .typescript := by
  constructor
  · rfl
  · intro ready
    exact Bool.noConfusion ready

theorem globalConjunctionMasksAReadyRoute :
    RouteReady rustReadyTypescriptRejected .rust ∧
    ¬ GlobalConjunctionReady rustReadyTypescriptRejected := by
  constructor
  · rfl
  · intro globallyReady
    exact Bool.noConfusion globallyReady.2

structure ResidentPhysicalGeneration where
  sessionId : Nat
  physicalGeneration : Nat

structure ResidentRetirementEvidence where
  retiredSessionId : Nat
  retiredGeneration : Nat
  registryRetired : Bool
  hostPathReleased : Bool

def RolloutAdoptable
    (candidate : ResidentPhysicalGeneration)
    (retirement : ResidentRetirementEvidence) : Prop :=
  candidate.sessionId ≠ retirement.retiredSessionId ∨
    candidate.physicalGeneration ≠ retirement.retiredGeneration

def ReplacementAdmissible
    (previous replacement : ResidentPhysicalGeneration)
    (retirement : ResidentRetirementEvidence) : Prop :=
  retirement.registryRetired = true ∧
    retirement.hostPathReleased = true ∧
    retirement.retiredSessionId = previous.sessionId ∧
    retirement.retiredGeneration = previous.physicalGeneration ∧
    previous.sessionId ≠ replacement.sessionId ∧
    previous.physicalGeneration < replacement.physicalGeneration

theorem registryRetirementAloneDoesNotProveHostPathRelease :
    ∃ evidence : ResidentRetirementEvidence,
      evidence.registryRetired = true ∧ evidence.hostPathReleased = false := by
  exact ⟨⟨1, 1, true, false⟩, rfl, rfl⟩

theorem retiredPhysicalGenerationCannotBeReadopted
    (generation : ResidentPhysicalGeneration)
    (retirement : ResidentRetirementEvidence)
    (sameSession : generation.sessionId = retirement.retiredSessionId)
    (sameGeneration : generation.physicalGeneration = retirement.retiredGeneration) :
    ¬ RolloutAdoptable generation retirement := by
  intro adoptable
  rcases adoptable with differentSession | differentGeneration
  · exact differentSession sameSession
  · exact differentGeneration sameGeneration

theorem admittedReplacementStrictlyAdvancesGeneration
    (previous replacement : ResidentPhysicalGeneration)
    (retirement : ResidentRetirementEvidence)
    (admitted : ReplacementAdmissible previous replacement retirement) :
    previous.physicalGeneration < replacement.physicalGeneration :=
  admitted.2.2.2.2.2

end ASPProof.AgentSessionLifecycleSpawnObservationGap
