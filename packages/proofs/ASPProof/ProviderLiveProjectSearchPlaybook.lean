-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

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

inductive GenerationDurabilityState where
  | absent
  | building
  | ready
  | failed
  deriving DecidableEq

structure GenerationPublicationReceipt where
  contentIdentityComplete : Bool
  residentReady : Bool
  durabilityState : GenerationDurabilityState

def AdmissibleGenerationReady (receipt : GenerationPublicationReceipt) : Prop :=
  receipt.contentIdentityComplete = true ∧ receipt.residentReady = true

def AdmissibleDurableRestore (receipt : GenerationPublicationReceipt) : Prop :=
  AdmissibleGenerationReady receipt ∧ receipt.durabilityState = .ready

def ResidentBeforeDurabilityReceipt : GenerationPublicationReceipt :=
  { contentIdentityComplete := true
    residentReady := true
    durabilityState := .building }

theorem resident_ready_does_not_wait_for_durability :
    AdmissibleGenerationReady ResidentBeforeDurabilityReceipt := by
  simp [AdmissibleGenerationReady, ResidentBeforeDurabilityReceipt]

theorem durability_failure_cannot_revoke_resident_ready
    (receipt : GenerationPublicationReceipt)
    (admissible : AdmissibleGenerationReady receipt) :
    AdmissibleGenerationReady { receipt with durabilityState := .failed } := by
  exact admissible

theorem durable_restore_requires_successful_attachment
    (receipt : GenerationPublicationReceipt)
    (restore : AdmissibleDurableRestore receipt) :
    receipt.durabilityState = .ready :=
  restore.2

def DurablePublicationBoundaryMs : Nat := 500

structure TimedGenerationPublicationReceipt extends GenerationPublicationReceipt where
  elapsedMs : Nat

def FastResidentPublicationReceipt : TimedGenerationPublicationReceipt :=
  { contentIdentityComplete := true
    residentReady := true
    durabilityState := .building
    elapsedMs := 1 }

/-- A fast resident publication is queryable but cannot be used as restart
restore evidence until the independently supervised attachment is durable. -/
theorem fast_resident_ready_is_not_yet_durable_restore :
    FastResidentPublicationReceipt.elapsedMs < DurablePublicationBoundaryMs ∧
      AdmissibleGenerationReady FastResidentPublicationReceipt.toGenerationPublicationReceipt ∧
      ¬ AdmissibleDurableRestore FastResidentPublicationReceipt.toGenerationPublicationReceipt := by
  constructor
  · decide
  · constructor
    · simp [FastResidentPublicationReceipt, AdmissibleGenerationReady]
    · simp [FastResidentPublicationReceipt, AdmissibleDurableRestore]

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

/-! The root Search Playbook composes Rust lexical/byte evidence for every
request and starts Python Graphs only for relationship-shaped intent.  The
optional worker is bound to the same immutable generation identity; it is not
a second publication authority. -/

inductive SearchPlaybookIntent where
  | lexical
  | exact
  | relationship
  | explicitGraph
  deriving DecidableEq

structure GraphSessionIdentity where
  projectId : String
  workspaceIdentity : String
  generationDigest : String
  rootDigest : String
  graphArtifactDigest : String
  deriving DecidableEq

inductive PythonGraphSelection where
  | skipped
  | exact (identity : GraphSessionIdentity)
  deriving DecidableEq

def RequiresPythonGraph : SearchPlaybookIntent → Bool
  | .relationship | .explicitGraph => true
  | .lexical | .exact => false

def AdmissiblePythonGraph
    (intent : SearchPlaybookIntent)
    (expected : GraphSessionIdentity)
    (selection : PythonGraphSelection) : Prop :=
  if RequiresPythonGraph intent then selection = .exact expected
  else selection = .skipped

def PythonWorkerStarts : PythonGraphSelection → Nat
  | .skipped => 0
  | .exact _ => 1

theorem lexical_and_exact_skip_python_without_worker_start
    (expected : GraphSessionIdentity) :
    AdmissiblePythonGraph .lexical expected .skipped ∧
      AdmissiblePythonGraph .exact expected .skipped ∧
      PythonWorkerStarts .skipped = 0 := by
  simp [AdmissiblePythonGraph, RequiresPythonGraph, PythonWorkerStarts]

theorem relationship_requires_the_exact_generation_identity
    (expected observed : GraphSessionIdentity)
    (admitted : AdmissiblePythonGraph .relationship expected (.exact observed)) :
    observed = expected := by
  simpa [AdmissiblePythonGraph, RequiresPythonGraph] using admitted

theorem stale_python_generation_fails_closed
    (expected observed : GraphSessionIdentity)
    (stale : observed ≠ expected) :
    ¬ AdmissiblePythonGraph .explicitGraph expected (.exact observed) := by
  simp [AdmissiblePythonGraph, RequiresPythonGraph, stale]

structure ResidentSearchTriad where
  lexicalIdentity : GraphSessionIdentity
  rustGraphIdentity : GraphSessionIdentity
  byteCoverageIdentity : GraphSessionIdentity
  deriving DecidableEq

def AdmissibleResidentSearchTriad
    (expected : GraphSessionIdentity)
    (triad : ResidentSearchTriad) : Prop :=
  triad.lexicalIdentity = expected ∧
    triad.rustGraphIdentity = expected ∧
    triad.byteCoverageIdentity = expected

theorem resident_search_triad_has_one_generation_identity
    (expected : GraphSessionIdentity)
    (triad : ResidentSearchTriad)
    (admitted : AdmissibleResidentSearchTriad expected triad) :
    triad.lexicalIdentity = triad.rustGraphIdentity ∧
      triad.rustGraphIdentity = triad.byteCoverageIdentity := by
  rcases admitted with ⟨lexical, graph, bytes⟩
  constructor <;> simp [lexical, graph, bytes]

structure WarmSearchWork where
  filesystemReads : Nat
  providerCalls : Nat
  processLaunches : Nat
  deriving DecidableEq

def ResidentWarmSearchWork (work : WarmSearchWork) : Prop :=
  work.filesystemReads = 0 ∧
    work.providerCalls = 0 ∧
    work.processLaunches = 0

theorem resident_warm_search_cannot_hide_external_work
    (work : WarmSearchWork)
    (resident : ResidentWarmSearchWork work) :
    work = { filesystemReads := 0, providerCalls := 0, processLaunches := 0 } := by
  rcases resident with ⟨reads, providers, processes⟩
  cases work
  simp_all

structure SearchPlaybookTimingReceipt where
  pythonWarmRankP99Nanos : Nat
  pythonReceiptValidationNanos : Nat

def PublicPlaybookDeadlineNanos : Nat := 500000000

def AdmissibleSearchPlaybookTiming (receipt : SearchPlaybookTimingReceipt) : Prop :=
  receipt.pythonWarmRankP99Nanos < PublicPlaybookDeadlineNanos

theorem python_execution_deadline_is_not_receipt_validation_time
    (receipt : SearchPlaybookTimingReceipt)
    (admitted : AdmissibleSearchPlaybookTiming receipt) :
    receipt.pythonWarmRankP99Nanos < 500000000 := by
  exact admitted

structure RgCoverageReceipt where
  workspaceIdentity : String
  generationDigest : String
  rootDigest : String
  byteArtifactDigest : String
  bytesCovered : Nat
  complete : Bool
  deriving DecidableEq

def AdmissibleRgCoverage
    (expectedWorkspace expectedGeneration expectedRoot expectedArtifact : String)
    (receipt : RgCoverageReceipt) : Prop :=
  receipt.workspaceIdentity = expectedWorkspace ∧
    receipt.generationDigest = expectedGeneration ∧
    receipt.rootDigest = expectedRoot ∧
    receipt.byteArtifactDigest = expectedArtifact ∧
    receipt.bytesCovered > 0 ∧
    receipt.complete = true

theorem boolean_rg_coverage_cannot_substitute_for_bound_receipt
    (expectedWorkspace expectedGeneration expectedRoot expectedArtifact : String)
    (receipt : RgCoverageReceipt)
    (admitted : AdmissibleRgCoverage expectedWorkspace expectedGeneration
      expectedRoot expectedArtifact receipt) :
    receipt.workspaceIdentity = expectedWorkspace ∧
      receipt.generationDigest = expectedGeneration ∧
      receipt.rootDigest = expectedRoot ∧
      receipt.byteArtifactDigest = expectedArtifact := by
  exact ⟨admitted.1, admitted.2.1, admitted.2.2.1, admitted.2.2.2.1⟩

theorem rg_artifact_drift_fails_closed
    (expectedWorkspace expectedGeneration expectedRoot expectedArtifact : String)
    (receipt : RgCoverageReceipt)
    (drift : receipt.byteArtifactDigest ≠ expectedArtifact) :
    ¬ AdmissibleRgCoverage expectedWorkspace expectedGeneration
      expectedRoot expectedArtifact receipt := by
  intro admitted
  exact drift admitted.2.2.2.1

structure FusedSearchCacheIdentity where
  projectId : String
  workspaceId : String
  sourceRootDigest : String
  generationDigest : String
  normalizedQueryTermsDigest : String
  profileDigest : String
  algorithmDigest : String
  lexicalFrontierDigest : String
  budgetDigest : String
  deriving DecidableEq

def FusedCacheHitAdmitted
    (expected observed : FusedSearchCacheIdentity) : Prop :=
  observed = expected

theorem fused_cache_cannot_cross_project_workspace_or_generation
    (expected observed : FusedSearchCacheIdentity)
    (drift : observed ≠ expected) :
    ¬ FusedCacheHitAdmitted expected observed := by
  exact drift

structure ContentGenerationBuildTerminal where
  fdInventoryJoined : Bool
  ownerBytesJoined : Bool
  contentIdentityComplete : Bool

inductive MutationGenerationRoute where
  | completeGeneration
  deriving DecidableEq

def MutationGenerationPublishable : MutationGenerationRoute → Bool
  | .completeGeneration => true

/-- Changed paths may prioritize inventory work, but cannot create an
owner-delta or overlay publication authority. -/
theorem mutation_has_only_complete_generation_publication
    (route : MutationGenerationRoute) :
    route = .completeGeneration := by
  cases route
  rfl

structure ProviderWorkHint where
  languageId : String
  providerId : String

inductive GenerationMembershipScope where
  | completeGeneration
  deriving DecidableEq

def MembershipScopeForHint (_hint : Option ProviderWorkHint) :
    GenerationMembershipScope :=
  .completeGeneration

/-- A provider hint may prioritize work but cannot narrow publication
membership or construct a partial generation. -/
theorem provider_hint_cannot_narrow_generation_membership
    (hint : Option ProviderWorkHint) :
    MembershipScopeForHint hint = .completeGeneration := by
  rfl

def ContentGenerationPublishable
    (terminal : ContentGenerationBuildTerminal) : Prop :=
  terminal.fdInventoryJoined = true ∧
    terminal.ownerBytesJoined = true ∧
    terminal.contentIdentityComplete = true

theorem content_generation_does_not_wait_for_lexical_acceleration
    (terminal : ContentGenerationBuildTerminal)
    (publishable : ContentGenerationPublishable terminal) :
    terminal.fdInventoryJoined = true ∧
      terminal.ownerBytesJoined = true ∧
      terminal.contentIdentityComplete = true := by
  exact publishable

inductive DerivedSearchAttachment where
  | tantivyLexical
  | residentGraph
  | pythonGraph
  deriving DecidableEq

def RequiredBeforeContentPublication : DerivedSearchAttachment → Bool
  | .tantivyLexical | .residentGraph | .pythonGraph => false

theorem content_publication_is_independent_of_all_derived_attachments :
    (∀ attachment, RequiredBeforeContentPublication attachment = false) := by
  intro attachment
  cases attachment <;> rfl

structure LexicalAcceleratorBuildTerminal where
  contentIdentityBound : Bool
  shardPlanComplete : Bool
  tantivyBuildJoined : Bool
  rgTantivyEquivalent : Bool

def LexicalAcceleratorPublishable
    (terminal : LexicalAcceleratorBuildTerminal) : Prop :=
  terminal.contentIdentityBound = true ∧
    terminal.shardPlanComplete = true ∧
    terminal.tantivyBuildJoined = true ∧
    terminal.rgTantivyEquivalent = true

theorem accelerator_cannot_publish_without_cold_route_equivalence
    (terminal : LexicalAcceleratorBuildTerminal)
    (publishable : LexicalAcceleratorPublishable terminal) :
    terminal.rgTantivyEquivalent = true := by
  exact publishable.2.2.2

/-! File discovery, cold rg execution, index construction, and serving are
distinct authorities. Content publication never waits for the accelerator;
the accelerator may become current only after equivalence validation. -/

structure LexicalGenerationPlanEvidence where
  admittedOwnerCount : Nat
  fdInventoryComplete : Bool
  contentGenerationBound : Bool
  lexicalFactCoverageComplete : Bool
  reusedShardCount : Nat
  rebuiltShardCount : Nat

def LexicalGenerationPublishable
    (evidence : LexicalGenerationPlanEvidence) : Prop :=
  evidence.fdInventoryComplete = true ∧
    evidence.contentGenerationBound = true ∧
    evidence.lexicalFactCoverageComplete = true ∧
    evidence.reusedShardCount + evidence.rebuiltShardCount =
      evidence.admittedOwnerCount

theorem lexical_generation_requires_complete_join
    (evidence : LexicalGenerationPlanEvidence)
    (publishable : LexicalGenerationPublishable evidence) :
      evidence.fdInventoryComplete = true ∧
      evidence.contentGenerationBound = true ∧
      evidence.lexicalFactCoverageComplete = true ∧
      evidence.reusedShardCount + evidence.rebuiltShardCount =
        evidence.admittedOwnerCount := by
  exact publishable

structure LexicalOpenEffects where
  fdProcessCount : Nat
  rgProcessCount : Nat
  tantivyBuildCount : Nat

def publishedLexicalOpenEffects : LexicalOpenEffects :=
  { fdProcessCount := 0
    rgProcessCount := 0
    tantivyBuildCount := 0 }

theorem published_lexical_open_is_build_free :
    publishedLexicalOpenEffects.fdProcessCount = 0 ∧
      publishedLexicalOpenEffects.rgProcessCount = 0 ∧
      publishedLexicalOpenEffects.tantivyBuildCount = 0 := by
  decide

def coldRgQueryEffects : LexicalOpenEffects :=
  { fdProcessCount := 0
    rgProcessCount := 1
    tantivyBuildCount := 0 }

theorem cold_route_reuses_inventory_and_never_builds_tantivy :
    coldRgQueryEffects.fdProcessCount = 0 ∧
      coldRgQueryEffects.rgProcessCount = 1 ∧
      coldRgQueryEffects.tantivyBuildCount = 0 := by
  decide

end ASPProof.ProviderLiveProjectSearchPlaybook
