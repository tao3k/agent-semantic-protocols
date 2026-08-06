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

structure RuntimeBinaryControlTopology where
  immutableDigestAddressedArtifact : Bool
  canonicalEntryIsAtomicPointer : Bool
  controlPathReadsBinaryBytes : Bool
  restartFramePrecedesSupervisorBoundary : Bool

def runtimeBinaryControlFastPath
    (topology : RuntimeBinaryControlTopology) : Prop :=
  topology.immutableDigestAddressedArtifact = true ∧
  topology.canonicalEntryIsAtomicPointer = true ∧
  topology.controlPathReadsBinaryBytes = false ∧
  topology.restartFramePrecedesSupervisorBoundary = true

theorem digest_addressed_pointer_keeps_binary_bytes_off_control_path
    (topology : RuntimeBinaryControlTopology)
    (immutable : topology.immutableDigestAddressedArtifact = true)
    (atomicPointer : topology.canonicalEntryIsAtomicPointer = true)
    (noByteRead : topology.controlPathReadsBinaryBytes = false)
    (frameWithinBoundary : topology.restartFramePrecedesSupervisorBoundary = true) :
    runtimeBinaryControlFastPath topology := by
  exact ⟨immutable, atomicPointer, noByteRead, frameWithinBoundary⟩

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

inductive GenerationOpenState where
  | ready
  | missing
  | recoveryRequired
  deriving DecidableEq, Repr

structure GenerationOpenTransition where
  daemonAdmissions : Nat
  pointerReopens : Nat
  queryReady : Bool
  typedBuilding : Bool
  deriving DecidableEq, Repr

def openGeneration : GenerationOpenState → GenerationOpenTransition
  | .ready =>
      { daemonAdmissions := 0, pointerReopens := 0, queryReady := true,
        typedBuilding := false }
  | .missing =>
      { daemonAdmissions := 1, pointerReopens := 1, queryReady := false,
        typedBuilding := true }
  | .recoveryRequired =>
      { daemonAdmissions := 0, pointerReopens := 0, queryReady := false,
        typedBuilding := false }

theorem missing_generation_has_one_bounded_admission_and_reopen :
    openGeneration .missing =
      { daemonAdmissions := 1, pointerReopens := 1, queryReady := false,
        typedBuilding := true } := by
  rfl

theorem recovery_required_is_fail_closed_without_retry :
    openGeneration .recoveryRequired =
      { daemonAdmissions := 0, pointerReopens := 0, queryReady := false,
        typedBuilding := false } := by
  rfl

structure ProviderRuntimeEntry where
  canonicalDigestLatticeSymlink : Bool
  binaryByteReads : Nat
  rewrites : Nat
  deriving DecidableEq, Repr

def providerRuntimeReconcileAllowed (entry : ProviderRuntimeEntry) : Prop :=
  entry.canonicalDigestLatticeSymlink = true ∧
    entry.binaryByteReads = 0 ∧ entry.rewrites = 0

theorem unmanaged_provider_runtime_entry_has_no_migration_path
    (entry : ProviderRuntimeEntry)
    (hUnmanaged : entry.canonicalDigestLatticeSymlink = false) :
    ¬ providerRuntimeReconcileAllowed entry := by
  intro hAllowed
  exact Bool.noConfusion (hUnmanaged.symm.trans hAllowed.1)

inductive ExactProjectionKind where
  | source
  | callableSkeleton
  | ownerItems
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

inductive ExactFreshnessStage where
  | preToolAdmission
  | queryDataPlane
  deriving DecidableEq, Repr

structure ExactOwnerReadTransition where
  stage : ExactFreshnessStage
  admittedContentDigest : Nat
  liveContentDigest : Nat
  reopenedOwnerContentDigest : Nat
  terminalReady : Bool
  pointerPublished : Bool
  generationReopened : Bool
  deriving DecidableEq, Repr

def preToolAdmissionClosed (transition : ExactOwnerReadTransition) : Prop :=
  transition.stage = .preToolAdmission ∧
    transition.terminalReady = true ∧ transition.pointerPublished = true ∧
      transition.generationReopened = true ∧
        transition.reopenedOwnerContentDigest = transition.liveContentDigest

def queryDataPlaneAllowed (transition : ExactOwnerReadTransition) : Prop :=
  transition.stage = .queryDataPlane ∧
    transition.admittedContentDigest = transition.liveContentDigest ∧
      transition.pointerPublished = true ∧ transition.generationReopened = true ∧
        transition.reopenedOwnerContentDigest = transition.admittedContentDigest

def handoffAdmissionToQuery
    (transition : ExactOwnerReadTransition) : ExactOwnerReadTransition :=
  { transition with
    stage := .queryDataPlane
    admittedContentDigest := transition.liveContentDigest }

theorem stale_query_cannot_repair_its_generation
    (transition : ExactOwnerReadTransition)
    (hStale : transition.admittedContentDigest ≠ transition.liveContentDigest) :
    ¬ queryDataPlaneAllowed transition := by
  intro hAllowed
  exact hStale hAllowed.2.1

theorem pretool_admission_handoff_is_query_ready
    (transition : ExactOwnerReadTransition)
    (hClosed : preToolAdmissionClosed transition) :
    queryDataPlaneAllowed (handoffAdmissionToQuery transition) := by
  rcases hClosed with ⟨_, _, hPublished, hReopened, hOwner⟩
  exact ⟨rfl, rfl, hPublished, hReopened, hOwner⟩

theorem query_data_plane_requires_a_published_pointer
    (transition : ExactOwnerReadTransition)
    (hAllowed : queryDataPlaneAllowed transition) :
    transition.pointerPublished = true := by
  exact hAllowed.2.2.1

theorem pretool_admission_is_not_a_query_read
    (transition : ExactOwnerReadTransition)
    (hAdmission : transition.stage = .preToolAdmission) :
    ¬ queryDataPlaneAllowed transition := by
  intro hAllowed
  exact ExactFreshnessStage.noConfusion (hAdmission.symm.trans hAllowed.1)

inductive MutationAdmissionObservation where
  | observed
  | lost
  deriving DecidableEq, Repr

theorem lost_mutation_does_not_relax_exact_read_freshness
    (kind : ExactProjectionKind)
    (evidence : ExactOwnerReadEvidence)
    (_mutation : MutationAdmissionObservation)
    (hAllowed : exactProjectionAllowed kind evidence) :
    ownerFresh evidence := by
  exact hAllowed

inductive OptionalProviderState where
  | unavailable
  | starting
  | healthy
  | draining
  | failed
  deriving DecidableEq, Repr

structure ServerStartupState where
  endpointPublished : Bool
  acceptLoopRunning : Bool
  providerState : OptionalProviderState
  deriving DecidableEq, Repr

inductive ServerStartupEvent where
  | publishEndpoint
  | startAcceptLoop
  | providerTransition (state : OptionalProviderState)
  deriving DecidableEq, Repr

def startupStep
    (state : ServerStartupState)
    (event : ServerStartupEvent) : ServerStartupState :=
  match event with
  | .publishEndpoint => { state with endpointPublished := true }
  | .startAcceptLoop =>
      { state with acceptLoopRunning := state.endpointPublished }
  | .providerTransition providerState => { state with providerState }

def coreReady (state : ServerStartupState) : Prop :=
  state.endpointPublished = true ∧ state.acceptLoopRunning = true

theorem optional_provider_transition_preserves_core_readiness
    (state : ServerStartupState)
    (providerState : OptionalProviderState)
    (hReady : coreReady state) :
    coreReady (startupStep state (.providerTransition providerState)) := by
  simpa [startupStep, coreReady] using hReady

theorem published_endpoint_without_accept_loop_is_not_ready
    (providerState : OptionalProviderState) :
    ¬ coreReady {
      endpointPublished := true
      acceptLoopRunning := false
      providerState := providerState
    } := by
  simp [coreReady]

structure CoreReadinessTiming where
  endpointPublishedMicros : Nat
  acceptLoopRunningMicros : Nat
  providerReadyMicros : Nat
  deriving DecidableEq, Repr

def coreReadinessWithinBudget (budgetMicros : Nat) (timing : CoreReadinessTiming) : Prop :=
  timing.acceptLoopRunningMicros - timing.endpointPublishedMicros ≤ budgetMicros

theorem optional_provider_timing_cannot_change_core_budget_verdict
    (budgetMicros : Nat)
    (timing : CoreReadinessTiming)
    (providerReadyMicros : Nat) :
    coreReadinessWithinBudget budgetMicros
      { timing with providerReadyMicros := providerReadyMicros } ↔
      coreReadinessWithinBudget budgetMicros timing := by
  rfl

inductive SupervisorDefinitionState where
  | unchanged
  | changed
  | absent
  deriving DecidableEq, Repr

inductive RestartAuthority where
  | serverControlPlane
  | platformSupervisor
  deriving DecidableEq, Repr

def restartAuthority : SupervisorDefinitionState → RestartAuthority
  | .unchanged => .serverControlPlane
  | .changed => .platformSupervisor
  | .absent => .platformSupervisor

theorem unchanged_definition_restart_is_server_owned :
    restartAuthority .unchanged = .serverControlPlane := by
  rfl

theorem platform_restart_requires_definition_change_or_absence
    (definition : SupervisorDefinitionState)
    (h : restartAuthority definition = .platformSupervisor) :
    definition = .changed ∨ definition = .absent := by
  cases definition <;> simp [restartAuthority] at h ⊢

structure StableRuntimeBinding where
  stableEntryIdentity : Nat
  artifactDigest : Nat
  deriving DecidableEq, Repr

def supervisorDefinitionIdentity (binding : StableRuntimeBinding) : Nat :=
  binding.stableEntryIdentity

theorem atomic_artifact_switch_preserves_supervisor_definition
    (binding : StableRuntimeBinding)
    (nextArtifactDigest : Nat) :
    supervisorDefinitionIdentity
        { binding with artifactDigest := nextArtifactDigest } =
      supervisorDefinitionIdentity binding := by
  rfl

inductive ReadLaneDecision where
  | reuseConnected
  | openEmpty
  | wait
  deriving DecidableEq, Repr

structure ReadLaneAvailability where
  selectedLaneConnected : Bool
  anyConnectedAvailable : Bool
  anyEmptyAvailable : Bool
  deriving DecidableEq, Repr

def legacyRoundRobinLaneDecision (availability : ReadLaneAvailability) : ReadLaneDecision :=
  if availability.selectedLaneConnected then
    .reuseConnected
  else if availability.anyEmptyAvailable then
    .openEmpty
  else
    .wait

def connectedFirstLaneDecision (availability : ReadLaneAvailability) : ReadLaneDecision :=
  if availability.anyConnectedAvailable then
    .reuseConnected
  else if availability.anyEmptyAvailable then
    .openEmpty
  else
    .wait

theorem connected_first_never_opens_when_connected_available
    (availability : ReadLaneAvailability)
    (hConnected : availability.anyConnectedAvailable = true) :
    connectedFirstLaneDecision availability = .reuseConnected := by
  simp [connectedFirstLaneDecision, hConnected]

theorem connected_first_opens_only_under_connection_pressure
    (availability : ReadLaneAvailability)
    (hOpen : connectedFirstLaneDecision availability = .openEmpty) :
    availability.anyConnectedAvailable = false ∧
      availability.anyEmptyAvailable = true := by
  cases hConnected : availability.anyConnectedAvailable <;>
    cases hEmpty : availability.anyEmptyAvailable <;>
    simp [connectedFirstLaneDecision, hConnected, hEmpty] at hOpen ⊢

example :
    legacyRoundRobinLaneDecision {
      selectedLaneConnected := false
      anyConnectedAvailable := true
      anyEmptyAvailable := true
    } = .openEmpty := by
  rfl

example :
    connectedFirstLaneDecision {
      selectedLaneConnected := false
      anyConnectedAvailable := true
      anyEmptyAvailable := true
    } = .reuseConnected := by
  rfl

inductive ActivityBookkeeping where
  | blockingMutexClock
  | atomicMonotonicClock
  deriving DecidableEq, Repr

def mayBlockReadHotPath : ActivityBookkeeping → Bool
  | .blockingMutexClock => true
  | .atomicMonotonicClock => false

theorem atomic_monotonic_activity_does_not_block_read_hot_path :
    mayBlockReadHotPath .atomicMonotonicClock = false := by
  rfl

inductive SearchAuthorityTransport where
  | unixSocket
  | sharedGenerationPointer
  deriving DecidableEq, Repr

def warmSchedulerRoundTrips : SearchAuthorityTransport → Nat
  | .unixSocket => 1
  | .sharedGenerationPointer => 0

theorem shared_generation_pointer_has_no_warm_scheduler_roundtrip :
    warmSchedulerRoundTrips .sharedGenerationPointer = 0 := by
  rfl

inductive GenerationPointerAvailability where
  | published
  | missing
  deriving DecidableEq, Repr

def coldAdmissionRoundTrips : GenerationPointerAvailability → Nat
  | .published => 0
  | .missing => 1

theorem published_pointer_requires_no_cold_admission_roundtrip :
    coldAdmissionRoundTrips .published = 0 := by
  rfl

def pointerMappingsForSessions (sessionCount : Nat) (sharedCatalog : Bool) : Nat :=
  if sessionCount = 0 then 0 else if sharedCatalog then 1 else sessionCount

theorem shared_pointer_catalog_maps_once
    (sessionCount : Nat)
    (hSessions : sessionCount > 0) :
    pointerMappingsForSessions sessionCount true = 1 := by
  simp [pointerMappingsForSessions, Nat.ne_of_gt hSessions]

structure LoadOnceMemoryBackendTopology where
  cellsPerPointerPath : Nat
  coldMmapOpensPerPointerGeneration : Nat
  warmMmapOpensPerQuery : Nat
  warmGenerationValidationsPerQuery : Nat
  warmDatabaseOpensPerQuery : Nat
  warmControlRoundTripsPerQuery : Nat
  crossWorkspaceOpenLock : Bool
  deriving DecidableEq, Repr

def loadOnceMemoryBackendClosed
    (topology : LoadOnceMemoryBackendTopology) : Prop :=
  topology.cellsPerPointerPath = 1 ∧
  topology.coldMmapOpensPerPointerGeneration ≤ 1 ∧
  topology.warmMmapOpensPerQuery = 0 ∧
  topology.warmGenerationValidationsPerQuery = 0 ∧
  topology.warmDatabaseOpensPerQuery = 0 ∧
  topology.warmControlRoundTripsPerQuery = 0 ∧
  topology.crossWorkspaceOpenLock = false

theorem concurrent_sessions_share_one_memory_backend_without_global_lock
    (topology : LoadOnceMemoryBackendTopology)
    (oneCell : topology.cellsPerPointerPath = 1)
    (coalescedCold : topology.coldMmapOpensPerPointerGeneration ≤ 1)
    (noWarmMmap : topology.warmMmapOpensPerQuery = 0)
    (noWarmValidation : topology.warmGenerationValidationsPerQuery = 0)
    (noWarmDatabase : topology.warmDatabaseOpensPerQuery = 0)
    (noWarmControl : topology.warmControlRoundTripsPerQuery = 0)
    (workspaceIsolation : topology.crossWorkspaceOpenLock = false) :
    loadOnceMemoryBackendClosed topology := by
  exact ⟨oneCell, coalescedCold, noWarmMmap, noWarmValidation,
    noWarmDatabase, noWarmControl, workspaceIsolation⟩

structure ResidentMutationFanoutTopology where
  gitDiscoveriesDuringAdmission : Nat
  originCandidateFromObservation : Bool
  nestedCandidateFromResidentEntry : Bool
  reusesOriginCandidateAcrossWorkspaceScopes : Bool
  crossWorkspaceWriterLock : Bool
  deriving DecidableEq, Repr

def residentMutationFanoutClosed
    (topology : ResidentMutationFanoutTopology) : Prop :=
  topology.gitDiscoveriesDuringAdmission = 0 ∧
  topology.originCandidateFromObservation = true ∧
  topology.nestedCandidateFromResidentEntry = true ∧
  topology.reusesOriginCandidateAcrossWorkspaceScopes = false ∧
  topology.crossWorkspaceWriterLock = false

theorem mutation_fanout_uses_resident_candidate_evidence_without_git_rediscovery
    (topology : ResidentMutationFanoutTopology)
    (noGitDiscovery : topology.gitDiscoveriesDuringAdmission = 0)
    (originEvidence : topology.originCandidateFromObservation = true)
    (nestedEvidence : topology.nestedCandidateFromResidentEntry = true)
    (scopeIsolation : topology.reusesOriginCandidateAcrossWorkspaceScopes = false)
    (writerIsolation : topology.crossWorkspaceWriterLock = false) :
    residentMutationFanoutClosed topology := by
  exact ⟨noGitDiscovery, originEvidence, nestedEvidence, scopeIsolation,
    writerIsolation⟩

structure MutationSubmissionFlightTopology where
  observerCount : Nat
  leaderCount : Nat
  candidateDiscoveryCount : Nat
  ipcAdmissionCount : Nat
  followersReturnCoalesced : Bool
  ensureReusesAcceptedCandidate : Bool
  deriving DecidableEq, Repr

def mutationSubmissionFlightClosed
    (topology : MutationSubmissionFlightTopology) : Prop :=
  topology.observerCount > 0 ∧
  topology.leaderCount = 1 ∧
  topology.candidateDiscoveryCount = 1 ∧
  topology.ipcAdmissionCount = 1 ∧
  topology.followersReturnCoalesced = true ∧
  topology.ensureReusesAcceptedCandidate = true

theorem equal_mutation_observers_have_one_accepted_leader
    (topology : MutationSubmissionFlightTopology)
    (hasObserver : topology.observerCount > 0)
    (oneLeader : topology.leaderCount = 1)
    (oneCandidateDiscovery : topology.candidateDiscoveryCount = 1)
    (oneIpcAdmission : topology.ipcAdmissionCount = 1)
    (followersCoalesce : topology.followersReturnCoalesced = true)
    (ensureReusesCandidate : topology.ensureReusesAcceptedCandidate = true) :
    mutationSubmissionFlightClosed topology := by
  exact ⟨hasObserver, oneLeader, oneCandidateDiscovery, oneIpcAdmission,
    followersCoalesce, ensureReusesCandidate⟩

structure PublishedPointerReadiness where
  pointerCommitted : Bool
  catalogMapped : Bool
  deriving DecidableEq, Repr

def pointerReadyForQueries (state : PublishedPointerReadiness) : Prop :=
  state.pointerCommitted = true ∧ state.catalogMapped = true

theorem unmapped_pointer_cannot_be_query_ready
    (pointerCommitted : Bool) :
    ¬ pointerReadyForQueries {
      pointerCommitted := pointerCommitted
      catalogMapped := false
    } := by
  simp [pointerReadyForQueries]

structure SearchAuthorityPointerEvidence where
  sourceRoot : Nat
  workspaceRoot : Nat
  sourceLeafCount : Nat
  workspaceLeafCount : Nat
  ownerCount : Nat
  deriving DecidableEq, Repr

def completeSearchAuthorityPointer
    (evidence : SearchAuthorityPointerEvidence) : Prop :=
  evidence.sourceRoot = evidence.workspaceRoot ∧
    evidence.sourceLeafCount = evidence.workspaceLeafCount ∧
    evidence.ownerCount ≤ evidence.workspaceLeafCount

theorem complete_pointer_refines_authority_binding
    (evidence : SearchAuthorityPointerEvidence)
    (hComplete : completeSearchAuthorityPointer evidence) :
    evidence.sourceRoot = evidence.workspaceRoot ∧
      evidence.sourceLeafCount = evidence.workspaceLeafCount := by
  exact ⟨hComplete.1, hComplete.2.1⟩

end ASPProof.ResidentMerkleFreshnessBudget
