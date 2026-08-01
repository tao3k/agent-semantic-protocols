namespace ASPProof.ExactSelectorGenerationAdmission

abbrev GenerationId := String
abbrev RootDigest := String
abbrev OwnerPath := String
abbrev StructuralSelector := String

structure SelectorRequest where
  ownerPath : OwnerPath
  structuralSelector : StructuralSelector
  deriving DecidableEq, Repr

/-- The Runtime Server publishes owner and selector membership as one immutable
generation. A selector read never combines inventories from two generations. -/
structure ActiveGeneration where
  generationId : GenerationId
  rootDigest : RootDigest
  owners : List OwnerPath
  selectors : List (OwnerPath × StructuralSelector)
  deriving DecidableEq, Repr

inductive Resolution where
  | resolved (generationId : GenerationId) (rootDigest : RootDigest)
  | selectorStale (generationId : GenerationId) (rootDigest : RootDigest)
  | itemMissing (generationId : GenerationId) (rootDigest : RootDigest)
  deriving DecidableEq, Repr

/-- Exact resolution is generation-first: owner membership is decided before
selector membership, and every outcome is bound to the active generation. -/
def resolve (active : ActiveGeneration) (request : SelectorRequest) : Resolution :=
  if request.ownerPath ∈ active.owners then
    if (request.ownerPath, request.structuralSelector) ∈ active.selectors then
      .resolved active.generationId active.rootDigest
    else
      .itemMissing active.generationId active.rootDigest
  else
    .selectorStale active.generationId active.rootDigest

def resolutionGeneration : Resolution → GenerationId
  | .resolved generationId _ => generationId
  | .selectorStale generationId _ => generationId
  | .itemMissing generationId _ => generationId

def resolutionRoot : Resolution → RootDigest
  | .resolved _ rootDigest => rootDigest
  | .selectorStale _ rootDigest => rootDigest
  | .itemMissing _ rootDigest => rootDigest

theorem every_resolution_is_active_generation_bound
    (active : ActiveGeneration)
    (request : SelectorRequest) :
    resolutionGeneration (resolve active request) = active.generationId ∧
      resolutionRoot (resolve active request) = active.rootDigest := by
  by_cases ownerPresent : request.ownerPath ∈ active.owners
  · by_cases selectorPresent :
      (request.ownerPath, request.structuralSelector) ∈ active.selectors
    · simp [resolve, ownerPresent, selectorPresent, resolutionGeneration, resolutionRoot]
    · simp [resolve, ownerPresent, selectorPresent, resolutionGeneration, resolutionRoot]
  · simp [resolve, ownerPresent, resolutionGeneration, resolutionRoot]

theorem absent_owner_is_selector_stale
    (active : ActiveGeneration)
    (request : SelectorRequest)
    (ownerAbsent : request.ownerPath ∉ active.owners) :
    resolve active request =
      .selectorStale active.generationId active.rootDigest := by
  simp [resolve, ownerAbsent]

theorem active_owner_absent_selector_is_item_missing
    (active : ActiveGeneration)
    (request : SelectorRequest)
    (ownerPresent : request.ownerPath ∈ active.owners)
    (selectorAbsent :
      (request.ownerPath, request.structuralSelector) ∉ active.selectors) :
    resolve active request = .itemMissing active.generationId active.rootDigest := by
  simp [resolve, ownerPresent, selectorAbsent]

theorem resolved_implies_active_owner_and_selector
    (active : ActiveGeneration)
    (request : SelectorRequest)
    (resolved : resolve active request =
      .resolved active.generationId active.rootDigest) :
    request.ownerPath ∈ active.owners ∧
      (request.ownerPath, request.structuralSelector) ∈ active.selectors := by
  simp only [resolve] at resolved
  split at resolved
  next ownerPresent =>
    split at resolved
    next selectorPresent => exact ⟨ownerPresent, selectorPresent⟩
    next => contradiction
  next => contradiction

inductive RecoveryRoute where
  | symbolSearch
  | ownerItems
  | none
  deriving DecidableEq, Repr

def recoveryRoute : Resolution → RecoveryRoute
  | .selectorStale _ _ => .symbolSearch
  | .itemMissing _ _ => .ownerItems
  | .resolved _ _ => .none

theorem stale_selector_never_reuses_absent_owner_authority
    (generationId : GenerationId)
    (rootDigest : RootDigest) :
    recoveryRoute (.selectorStale generationId rootDigest) = .symbolSearch := by
  rfl

structure RuntimeState where
  active : ActiveGeneration
  deriving DecidableEq, Repr

def publish (state : RuntimeState) (next : ActiveGeneration) : RuntimeState :=
  { state with active := next }

theorem publication_replaces_generation_and_selector_inventory_atomically
    (state : RuntimeState)
    (next : ActiveGeneration) :
    (publish state next).active.generationId = next.generationId ∧
      (publish state next).active.rootDigest = next.rootDigest ∧
      (publish state next).active.owners = next.owners ∧
      (publish state next).active.selectors = next.selectors := by
  simp [publish]

/-- Admission acceptance and generation readiness are distinct lifecycle
states. PreToolUse may release an exact query only after the typed ensure
receipt carries the complete active generation. -/
inductive AdmissionState where
  | building
  | ready (active : ActiveGeneration)
  | failed
  deriving DecidableEq, Repr

inductive PreToolDecision where
  | continueWith (active : ActiveGeneration)
  | deny
  deriving DecidableEq, Repr

def admissionAccepts : AdmissionState → Bool
  | .building => false
  | .ready _ => true
  | .failed => true

/-- Building is the only single-flight exclusion state. Ready is an observed
generation, not a permanent cache lock: a later PreTool admission must be able
to run the incremental builder and publish a successor generation. -/
theorem ready_generation_does_not_block_incremental_readmission
    (active : ActiveGeneration) :
    admissionAccepts (.ready active) = true := by
  rfl

theorem building_generation_preserves_single_flight :
    admissionAccepts .building = false := by
  rfl

inductive AdmissionOperation where
  | admitSuccessor
  | ensureCurrent
  | restoreCurrent
  deriving DecidableEq, Repr

def schedulesBuild : AdmissionOperation → AdmissionState → Bool
  | .admitSuccessor, .ready _ => true
  | .admitSuccessor, .failed => true
  | .admitSuccessor, .building => false
  | .ensureCurrent, _ => false
  | .restoreCurrent, _ => false

theorem ensure_ready_generation_is_observation_only
    (active : ActiveGeneration) :
    schedulesBuild .ensureCurrent (.ready active) = false := by
  rfl

theorem restore_ready_generation_is_idempotent
    (active : ActiveGeneration) :
    schedulesBuild .restoreCurrent (.ready active) = false := by
  rfl

theorem explicit_admission_can_publish_ready_successor
    (active : ActiveGeneration) :
    schedulesBuild .admitSuccessor (.ready active) = true := by
  rfl

def finishPreToolAdmission (ensured : AdmissionState) : PreToolDecision :=
  match ensured with
  | .ready active => .continueWith active
  | .building => .deny
  | .failed => .deny

def exactQueryEnabled : PreToolDecision → Bool
  | .continueWith _ => true
  | .deny => false

theorem building_admission_never_releases_exact_query :
    exactQueryEnabled (finishPreToolAdmission .building) = false := by
  rfl

theorem failed_admission_never_releases_exact_query :
    exactQueryEnabled (finishPreToolAdmission .failed) = false := by
  rfl

theorem only_ready_admission_releases_its_active_generation
    (active : ActiveGeneration) :
    finishPreToolAdmission (.ready active) = .continueWith active := by
  rfl

abbrev WorkspaceId := String
abbrev RepositoryId := String
abbrev WorktreeId := String
abbrev SessionId := String
abbrev DbOwnerId := String
abbrev ProviderId := String
abbrev ScopeDigest := String

/-- A workspace is identified by repository plus worktree. Provider project
scope is deliberately absent from identity. -/
structure WorktreeWorkspace where
  workspaceId : WorkspaceId
  repositoryId : RepositoryId
  worktreeId : WorktreeId
  deriving DecidableEq, Repr

/-- The resident runtime has exactly one DB owner field while accepting any
number of client sessions. Registering a session cannot allocate a new owner. -/
structure WorkspaceRuntime where
  workspace : WorktreeWorkspace
  dbOwnerId : DbOwnerId
  sessions : List SessionId
  deriving DecidableEq, Repr

def registerSession (runtime : WorkspaceRuntime) (sessionId : SessionId) : WorkspaceRuntime :=
  if sessionId ∈ runtime.sessions then runtime
  else { runtime with sessions := sessionId :: runtime.sessions }

def sessionOwner (runtime : WorkspaceRuntime) (sessionId : SessionId) : Option DbOwnerId :=
  if sessionId ∈ runtime.sessions then some runtime.dbOwnerId else none

theorem session_registration_preserves_the_unique_owner
    (runtime : WorkspaceRuntime)
    (sessionId : SessionId) :
    (registerSession runtime sessionId).dbOwnerId = runtime.dbOwnerId := by
  unfold registerSession
  split <;> rfl

theorem register_session_contains_the_session
    (runtime : WorkspaceRuntime)
    (sessionId : SessionId) :
    sessionId ∈ (registerSession runtime sessionId).sessions := by
  unfold registerSession
  split <;> simp_all

theorem register_session_preserves_existing_sessions
    (runtime : WorkspaceRuntime)
    (sessionId existing : SessionId)
    (present : existing ∈ runtime.sessions) :
    existing ∈ (registerSession runtime sessionId).sessions := by
  unfold registerSession
  split <;> simp_all

theorem two_sessions_share_the_same_workspace_owner
    (runtime : WorkspaceRuntime)
    (left right : SessionId) :
    let registered := registerSession (registerSession runtime left) right
    sessionOwner registered left = some runtime.dbOwnerId ∧
      sessionOwner registered right = some runtime.dbOwnerId := by
  let once := registerSession runtime left
  let registered := registerSession once right
  have leftInOnce : left ∈ once.sessions :=
    register_session_contains_the_session runtime left
  have leftInRegistered : left ∈ registered.sessions :=
    register_session_preserves_existing_sessions once right left leftInOnce
  have rightInRegistered : right ∈ registered.sessions :=
    register_session_contains_the_session once right
  have onceOwner : once.dbOwnerId = runtime.dbOwnerId :=
    session_registration_preserves_the_unique_owner runtime left
  have registeredOwner : registered.dbOwnerId = runtime.dbOwnerId := by
    calc
      registered.dbOwnerId = once.dbOwnerId :=
        session_registration_preserves_the_unique_owner once right
      _ = runtime.dbOwnerId := onceOwner
  change sessionOwner registered left = some runtime.dbOwnerId ∧
    sessionOwner registered right = some runtime.dbOwnerId
  constructor
  · simp [sessionOwner, leftInRegistered, registeredOwner]
  · simp [sessionOwner, rightInRegistered, registeredOwner]

/-- Provider resolution is a generation-bound scope receipt for an already
identified workspace; it is not an identity constructor. -/
structure ProviderScopeReceipt where
  workspaceId : WorkspaceId
  providerId : ProviderId
  scopeDigest : ScopeDigest
  generationId : GenerationId
  deriving DecidableEq, Repr

structure PublishedProviderScope where
  workspace : WorktreeWorkspace
  receipt : ProviderScopeReceipt
  deriving DecidableEq, Repr

def publishProviderScope
    (workspace : WorktreeWorkspace)
    (receipt : ProviderScopeReceipt) : Option PublishedProviderScope :=
  if receipt.workspaceId = workspace.workspaceId then
    some { workspace, receipt }
  else
    none

theorem provider_scope_publication_preserves_workspace_identity
    (workspace : WorktreeWorkspace)
    (receipt : ProviderScopeReceipt)
    (admitted : receipt.workspaceId = workspace.workspaceId) :
    (publishProviderScope workspace receipt).map (·.workspace) = some workspace := by
  simp [publishProviderScope, admitted]

theorem provider_scope_digest_cannot_create_a_workspace_identity
    (workspace : WorktreeWorkspace)
    (left right : ProviderScopeReceipt)
    (leftAdmitted : left.workspaceId = workspace.workspaceId)
    (rightAdmitted : right.workspaceId = workspace.workspaceId) :
    (publishProviderScope workspace left).map (·.workspace.workspaceId) =
      (publishProviderScope workspace right).map (·.workspace.workspaceId) := by
  simp [publishProviderScope, leftAdmitted, rightAdmitted]

end ASPProof.ExactSelectorGenerationAdmission
