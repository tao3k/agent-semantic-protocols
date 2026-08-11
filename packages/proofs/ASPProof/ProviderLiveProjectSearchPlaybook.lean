namespace ASPProof.ProviderLiveProjectSearchPlaybook

structure ProviderCapability where
  providerId : String
  languageId : String
  searchable : Bool
  hasLiveFixture : Bool
  ownerBackedFixture : Bool
  declaredRoutes : List String

def RequiresCoverage (provider : ProviderCapability) : Prop :=
  provider.searchable = true

def Covered (provider : ProviderCapability) : Prop :=
  provider.hasLiveFixture = true ∧
    provider.ownerBackedFixture = true ∧
    ∀ route, route ∈ provider.declaredRoutes → route ≠ ""

def RegistryCovered (registry : List ProviderCapability) : Prop :=
  ∀ provider, provider ∈ registry → RequiresCoverage provider → Covered provider

theorem registered_searchable_provider_cannot_escape_matrix
    (registry : List ProviderCapability)
    (covered : RegistryCovered registry)
    (provider : ProviderCapability)
    (member : provider ∈ registry)
    (searchable : provider.searchable = true) :
    Covered provider :=
  covered provider member searchable

theorem covered_provider_cannot_use_standalone_temporary_checkout
    (provider : ProviderCapability)
    (covered : Covered provider) :
    provider.ownerBackedFixture = true :=
  covered.2.1

inductive AttemptState where
  | ready
  | ipcPermissionDenied
  | generationRequired
  | searchIoTimedOut
  | succeeded
  deriving DecidableEq

def Terminal : AttemptState → Prop
  | .ipcPermissionDenied => True
  | .generationRequired => True
  | .searchIoTimedOut => True
  | .succeeded => True
  | .ready => False

def RecursivelyDispatches : AttemptState → Bool
  | .ready => false
  | .ipcPermissionDenied => false
  | .generationRequired => false
  | .searchIoTimedOut => false
  | .succeeded => false

theorem typed_search_failures_terminate_without_recursive_dispatch
    (state : AttemptState)
    (failure : state = .ipcPermissionDenied ∨
      state = .generationRequired ∨ state = .searchIoTimedOut) :
    Terminal state ∧ RecursivelyDispatches state = false := by
  rcases failure with rfl | rfl | rfl <;> trivial

inductive ExecutionAuthority where
  | sandbox
  | hostNative
  deriving DecidableEq

def NextExecutionAuthority : AttemptState → ExecutionAuthority
  | .ipcPermissionDenied => .hostNative
  | _ => .sandbox

def RepairsRuntime : AttemptState → Bool
  | .ipcPermissionDenied => false
  | _ => false

def StartsGenerationAdmission : AttemptState → Bool
  | _ => false

theorem ipc_permission_denial_transfers_exact_request_without_recursion :
    NextExecutionAuthority .ipcPermissionDenied = .hostNative ∧
      RecursivelyDispatches .ipcPermissionDenied = false ∧
      RepairsRuntime .ipcPermissionDenied = false := by
  decide

theorem sandbox_retry_is_not_the_ipc_permission_recovery_authority :
    NextExecutionAuthority .ipcPermissionDenied ≠ .sandbox := by
  decide

theorem missing_generation_query_neither_waits_nor_starts_admission :
    Terminal .generationRequired ∧
      StartsGenerationAdmission .generationRequired = false ∧
      RecursivelyDispatches .generationRequired = false := by
  constructor
  · trivial
  · constructor <;> rfl

inductive QueryCapability where
  | deriveStableWorkspaceIdentity
  | readPublishedGeneration
  | directDatabaseOpen
  | readAdmissionCatalog
  | bootstrapGeneration
  | reconcileGeneration
  deriving DecidableEq

def NewQueryDataPlaneAllows : QueryCapability → Bool
  | .deriveStableWorkspaceIdentity => true
  | .readPublishedGeneration => true
  | .directDatabaseOpen => false
  | .readAdmissionCatalog => false
  | .bootstrapGeneration => false
  | .reconcileGeneration => false

/-- The new query plane has no compatibility edge back into the former
catalog/bootstrap/reconcile control flow. -/
theorem query_plane_has_no_legacy_generation_control :
    NewQueryDataPlaneAllows .directDatabaseOpen = false ∧
      NewQueryDataPlaneAllows .readAdmissionCatalog = false ∧
      NewQueryDataPlaneAllows .bootstrapGeneration = false ∧
      NewQueryDataPlaneAllows .reconcileGeneration = false := by
  decide

theorem query_plane_retains_only_identity_and_immutable_generation_reads :
    NewQueryDataPlaneAllows .deriveStableWorkspaceIdentity = true ∧
      NewQueryDataPlaneAllows .readPublishedGeneration = true := by
  decide

inductive GenerationControlSurface where
  | hookMutationSubmission
  | lifecycleMutationAdmission
  | operatorCacheControl
  | searchQuery
  deriving DecidableEq

def AwaitsTerminalGeneration : GenerationControlSurface → Bool
  | .hookMutationSubmission => false
  | .lifecycleMutationAdmission => true
  | .operatorCacheControl => true
  | .searchQuery => false

theorem only_admission_control_awaits_terminal_generation :
    AwaitsTerminalGeneration .hookMutationSubmission = false ∧
      AwaitsTerminalGeneration .lifecycleMutationAdmission = true ∧
      AwaitsTerminalGeneration .operatorCacheControl = true ∧
      AwaitsTerminalGeneration .searchQuery = false := by
  decide

inductive MutationSubmissionRole where
  | firstPublisher
  | identicalDuplicate
  deriving DecidableEq

def RequiresExclusivePublication : MutationSubmissionRole → Bool
  | .firstPublisher => true
  | .identicalDuplicate => false

def WaitsForGenerationTerminal : MutationSubmissionRole → Bool
  | _ => false

/-- The atomic absent-to-present transition is the only writer authority.
Identical concurrent submissions remain on the shared-read coalescing path. -/
theorem identical_duplicate_cannot_enter_exclusive_publication :
    RequiresExclusivePublication .identicalDuplicate = false := by
  rfl

/-- Submission is an acknowledgement boundary, not generation admission. -/
theorem mutation_submission_never_waits_for_generation_terminal
    (role : MutationSubmissionRole) :
    WaitsForGenerationTerminal role = false := by
  cases role <;> rfl

structure AttemptScopedGenerationReceipt where
  expectedAttempt : Nat
  observedAttempt : Nat
  terminal : Bool

def SatisfiesExpectedAttempt (receipt : AttemptScopedGenerationReceipt) : Prop :=
  receipt.observedAttempt = receipt.expectedAttempt ∧ receipt.terminal = true

/-- A terminal receipt from another generation attempt cannot complete the
explicit lifecycle admission that owns the expected attempt. -/
theorem different_attempt_cannot_satisfy_lifecycle_admission
    (receipt : AttemptScopedGenerationReceipt)
    (different : receipt.observedAttempt ≠ receipt.expectedAttempt) :
    ¬ SatisfiesExpectedAttempt receipt := by
  intro satisfies
  exact different satisfies.1

theorem lifecycle_admission_returns_its_attempt_terminal
    (receipt : AttemptScopedGenerationReceipt)
    (satisfies : SatisfiesExpectedAttempt receipt) :
    receipt.observedAttempt = receipt.expectedAttempt ∧ receipt.terminal = true :=
  satisfies

inductive PublishedPointerState where
  | missing
  | recoveryRequired
  | ready
  deriving DecidableEq

structure GenerationPublicationReceipt where
  controlReady : Bool
  pointerState : PublishedPointerState

def AdmissibleGenerationReady (receipt : GenerationPublicationReceipt) : Prop :=
  receipt.controlReady = true ∧ receipt.pointerState = .ready

/-- The former resident-first writer could emit control-plane `Ready` while
the immutable pointer was still absent.  The new admission relation rejects
that concrete counterexample. -/
def OldResidentFirstCounterexample : GenerationPublicationReceipt :=
  { controlReady := true, pointerState := .missing }

theorem old_resident_first_ready_is_not_admissible :
    ¬ AdmissibleGenerationReady OldResidentFirstCounterexample := by
  simp [AdmissibleGenerationReady, OldResidentFirstCounterexample]

theorem admissible_ready_implies_published_pointer
    (receipt : GenerationPublicationReceipt)
    (admissible : AdmissibleGenerationReady receipt) :
    receipt.pointerState = .ready :=
  admissible.2

theorem missing_pointer_cannot_return_ready
    (receipt : GenerationPublicationReceipt)
    (missing : receipt.pointerState = .missing) :
    ¬ AdmissibleGenerationReady receipt := by
  intro admissible
  have impossible : PublishedPointerState.missing = .ready :=
    missing.symm.trans admissible.2
  exact PublishedPointerState.noConfusion impossible

def DurablePublicationBoundaryMs : Nat := 500

structure TimedGenerationPublicationReceipt extends GenerationPublicationReceipt where
  elapsedMs : Nat

def LegacyFastResidentOnlyReceipt : TimedGenerationPublicationReceipt :=
  { controlReady := true, pointerState := .missing, elapsedMs := 1 }

/-- Meeting the former one-millisecond resident-only timing gate cannot prove
durable publication.  This concrete fast receipt still lacks its pointer. -/
theorem legacy_fast_timing_cannot_imply_durable_ready :
    LegacyFastResidentOnlyReceipt.elapsedMs < DurablePublicationBoundaryMs ∧
      ¬ AdmissibleGenerationReady LegacyFastResidentOnlyReceipt.toGenerationPublicationReceipt := by
  constructor
  · decide
  · simp [LegacyFastResidentOnlyReceipt, AdmissibleGenerationReady]

def RuntimeIpcTypicalBudgetMs : Nat := 50
def RuntimeIpcHardBoundaryMs : Nat := 500
def ResidentDataPlaneTypicalBudgetMicros : Nat := 1000
def ResidentDataPlaneHardBoundaryMicros : Nat := 10000
def DurableHookRecoveryTypicalBudgetMs : Nat := 50
def DurableHookRecoveryHardBoundaryMs : Nat := 500

theorem runtime_ipc_typical_budget_is_strictly_inside_hard_boundary :
    RuntimeIpcTypicalBudgetMs < RuntimeIpcHardBoundaryMs := by
  decide

theorem runtime_ipc_typical_success_is_inside_hard_boundary
    (elapsedMs : Nat)
    (typical : elapsedMs < RuntimeIpcTypicalBudgetMs) :
    elapsedMs < RuntimeIpcHardBoundaryMs := by
  exact Nat.lt_trans typical runtime_ipc_typical_budget_is_strictly_inside_hard_boundary

theorem resident_data_plane_typical_budget_is_strictly_inside_hard_boundary :
    ResidentDataPlaneTypicalBudgetMicros < ResidentDataPlaneHardBoundaryMicros := by
  decide

theorem durable_hook_recovery_budget_is_not_hook_decision_budget :
    DurableHookRecoveryTypicalBudgetMs < DurableHookRecoveryHardBoundaryMs := by
  decide

structure RestartPublication where
  drainingReceiptFlushed : Bool
  responseHalfClosed : Bool
  processDrainPublished : Bool

def AdmissibleRestartPublication (publication : RestartPublication) : Prop :=
  publication.drainingReceiptFlushed = true ∧
    publication.responseHalfClosed = true ∧
    publication.processDrainPublished = true

def OldDrainFirstRestart : RestartPublication :=
  { drainingReceiptFlushed := false
    responseHalfClosed := false
    processDrainPublished := true }

theorem drain_first_restart_is_not_admissible :
    ¬ AdmissibleRestartPublication OldDrainFirstRestart := by
  simp [AdmissibleRestartPublication, OldDrainFirstRestart]

theorem admissible_restart_flushes_receipt_before_process_drain
    (publication : RestartPublication)
    (admissible : AdmissibleRestartPublication publication) :
    publication.drainingReceiptFlushed = true ∧
      publication.responseHalfClosed = true :=
  ⟨admissible.1, admissible.2.1⟩

inductive SearchExecutionCapability where
  | tokioAsyncIo
  | tokioDeadline
  | tokioCancellation
  | tokioSingleFlight
  | boundedCpuOffload
  | synchronousThread
  | nestedBlockOn
  | manualSleepPolling
  | queryRuntimeRepair
  deriving DecidableEq

def NewSearchExecutionAllows : SearchExecutionCapability → Bool
  | .tokioAsyncIo => true
  | .tokioDeadline => true
  | .tokioCancellation => true
  | .tokioSingleFlight => true
  | .boundedCpuOffload => true
  | .synchronousThread => false
  | .nestedBlockOn => false
  | .manualSleepPolling => false
  | .queryRuntimeRepair => false

/-- Search and exact query are Tokio-native consumers of an immutable
generation.  No synchronous compatibility bridge can regain control. -/
theorem search_execution_has_no_legacy_sync_control :
    NewSearchExecutionAllows .synchronousThread = false ∧
      NewSearchExecutionAllows .nestedBlockOn = false ∧
      NewSearchExecutionAllows .manualSleepPolling = false ∧
      NewSearchExecutionAllows .queryRuntimeRepair = false := by
  decide

theorem search_execution_retains_tokio_and_bounded_cpu_capabilities :
    NewSearchExecutionAllows .tokioAsyncIo = true ∧
      NewSearchExecutionAllows .tokioDeadline = true ∧
      NewSearchExecutionAllows .tokioCancellation = true ∧
      NewSearchExecutionAllows .tokioSingleFlight = true ∧
      NewSearchExecutionAllows .boundedCpuOffload = true := by
  decide

inductive SearchConnectionLane where
  | reusable
  | partialFrame
  | discarded
  deriving DecidableEq

def OnSearchIoTimeout : SearchConnectionLane → SearchConnectionLane
  | _ => .discarded

def ReusableAfterTimeout (lane : SearchConnectionLane) : Bool :=
  OnSearchIoTimeout lane == .reusable

/-- Cancellation cannot publish a partially consumed frame back into the
connection pool. -/
theorem timed_out_search_lane_is_never_reused (lane : SearchConnectionLane) :
    OnSearchIoTimeout lane = .discarded ∧ ReusableAfterTimeout lane = false := by
  cases lane <;> decide

inductive LiveProjectFixtureCapability where
  | gixOwnerDiscovery
  | isolatedOwnerBackedRepository
  | isolatedStateHome
  | inheritedHostStateHome
  | operatingSystemTemporaryCheckout
  | shellWorkspaceInitialization
  | workspaceWideStaticScan
  deriving DecidableEq

def LiveProjectFixtureAllows : LiveProjectFixtureCapability → Bool
  | .gixOwnerDiscovery => true
  | .isolatedOwnerBackedRepository => true
  | .isolatedStateHome => true
  | .inheritedHostStateHome => false
  | .operatingSystemTemporaryCheckout => false
  | .shellWorkspaceInitialization => false
  | .workspaceWideStaticScan => false

/-- Live-project scenarios are atomic provider fixtures derived from repository
authority.  Neither an OS temporary checkout nor a shell/static-scan fallback
can regain control of the testing framework. -/
theorem live_project_fixture_has_no_legacy_workspace_materialization :
    LiveProjectFixtureAllows .operatingSystemTemporaryCheckout = false ∧
      LiveProjectFixtureAllows .shellWorkspaceInitialization = false ∧
      LiveProjectFixtureAllows .workspaceWideStaticScan = false := by
  decide

theorem live_project_fixture_uses_gix_and_owner_backed_isolation :
    LiveProjectFixtureAllows .gixOwnerDiscovery = true ∧
      LiveProjectFixtureAllows .isolatedOwnerBackedRepository = true ∧
      LiveProjectFixtureAllows .isolatedStateHome = true := by
  decide

/-- A live-project fixture cannot map or truncate the Host's resident Runtime
artifacts; doing so would make concurrent test cleanup capable of SIGBUS. -/
theorem live_project_fixture_cannot_inherit_host_state_home :
    LiveProjectFixtureAllows .inheritedHostStateHome = false := by
  rfl

structure RecoveryPublication where
  workspaceIdentity : String
  generation : Nat
  published : Bool

def AdmissibleRecovery (receipt : RecoveryPublication) : Prop :=
  receipt.workspaceIdentity ≠ "" ∧ receipt.generation > 0 ∧ receipt.published = true

theorem recovery_is_independent_of_failed_attempt
    (receipt : RecoveryPublication)
    (admissible : AdmissibleRecovery receipt) :
    receipt.generation > 0 ∧ receipt.published = true :=
  ⟨admissible.2.1, admissible.2.2⟩

inductive RuntimeStartupDependency where
  | endpointTransport
  | workspaceStore
  | hookLocalMatcher
  | graphTurboSidecar
  deriving DecidableEq

def RequiredBeforeEndpoint : RuntimeStartupDependency → Bool
  | .endpointTransport => true
  | .workspaceStore => true
  | .hookLocalMatcher => false
  | .graphTurboSidecar => false

theorem hook_matcher_cannot_block_runtime_endpoint_publication :
    RequiredBeforeEndpoint .hookLocalMatcher = false := by
  rfl

theorem optional_graph_sidecar_cannot_block_runtime_endpoint_publication :
    RequiredBeforeEndpoint .graphTurboSidecar = false := by
  rfl

inductive HookLocalCapability where
  | localPolicySnapshot
  | localMemorySnapshot
  | runtimeControlProbe
  | synchronousConnectWorker
  deriving DecidableEq

def NewHookPlaneAllows : HookLocalCapability → Bool
  | .localPolicySnapshot => true
  | .localMemorySnapshot => true
  | .runtimeControlProbe => false
  | .synchronousConnectWorker => false

/-- Hook evaluation is a one-shot local policy operation; it cannot regain a
dependency on Runtime liveness or thread-per-connect supervision. -/
theorem hook_plane_has_no_runtime_probe_or_sync_worker :
    NewHookPlaneAllows .runtimeControlProbe = false ∧
      NewHookPlaneAllows .synchronousConnectWorker = false := by
  decide

end ASPProof.ProviderLiveProjectSearchPlaybook
