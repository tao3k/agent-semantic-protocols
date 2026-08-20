namespace ASPProof.IncrementalCacheAuthority

inductive GenerationState where
  | missing
  | building
  | failed
  | cancelled
  | ready
  deriving DecidableEq, Repr

inductive BuildOwner where
  | runtimeWorkspaceRegistry
  | ipcRequest
  deriving DecidableEq, Repr

inductive BuildEvent where
  | requestDisconnected
  | runtimeShutdown
  | superseded
  | leaseExpired
  deriving DecidableEq, Repr

def buildContinues (owner : BuildOwner) (event : BuildEvent) : Bool :=
  match owner, event with
  | .runtimeWorkspaceRegistry, .requestDisconnected => true
  | .runtimeWorkspaceRegistry, _ => false
  | .ipcRequest, _ => false

theorem runtime_owned_build_survives_request_disconnect :
    buildContinues .runtimeWorkspaceRegistry .requestDisconnected = true := by
  rfl

theorem request_cannot_be_the_build_owner
    (owner : BuildOwner)
    (h : buildContinues owner .requestDisconnected = true) :
    owner = .runtimeWorkspaceRegistry := by
  cases owner <;> simp_all [buildContinues]

inductive QueryEffect where
  | openResidentMmap
  | openTurso
  | activateProvider
  | submitAdmission
  deriving DecidableEq, Repr

def queryEffects : GenerationState → List QueryEffect
  | .ready => [.openResidentMmap]
  | .missing | .building | .failed | .cancelled => []

theorem non_ready_query_has_no_effects
    (state : GenerationState)
    (h : state ≠ .ready) :
    queryEffects state = [] := by
  cases state <;> simp_all [queryEffects]

theorem query_never_opens_turso_or_activates_provider
    (state : GenerationState) :
    .openTurso ∉ queryEffects state ∧
      .activateProvider ∉ queryEffects state ∧
      .submitAdmission ∉ queryEffects state := by
  cases state <;> simp [queryEffects]

def exactQueryEffects
    (state : GenerationState)
    (selectorAdmitted : Bool) : List QueryEffect :=
  if state = .ready ∧ selectorAdmitted = true then [.openResidentMmap] else []

theorem ready_empty_capability_fails_before_mmap :
    exactQueryEffects .ready false = [] := by
  rfl

theorem exact_query_opens_mmap_only_with_same_generation_capability
    (state : GenerationState)
    (selectorAdmitted : Bool)
    (h : .openResidentMmap ∈ exactQueryEffects state selectorAdmitted) :
    state = .ready ∧ selectorAdmitted = true := by
  unfold exactQueryEffects at h
  split at h
  · assumption
  · simp at h

structure GenerationAuthorityProjection where
  cacheStatus : GenerationState
  queryAdmission : GenerationState
  pointerState : GenerationState
  deriving DecidableEq, Repr

def publishAuthorityState (state : GenerationState) : GenerationAuthorityProjection :=
  { cacheStatus := state, queryAdmission := state, pointerState := state }

theorem one_publication_has_no_split_generation_state
    (state : GenerationState) :
    let projection := publishAuthorityState state
    projection.cacheStatus = projection.queryAdmission ∧
      projection.queryAdmission = projection.pointerState := by
  constructor <;> rfl

theorem non_ready_publication_cannot_expose_a_ready_pointer
    (state : GenerationState)
    (h : state ≠ .ready) :
    (publishAuthorityState state).pointerState ≠ .ready := by
  simpa [publishAuthorityState] using h

end ASPProof.IncrementalCacheAuthority
