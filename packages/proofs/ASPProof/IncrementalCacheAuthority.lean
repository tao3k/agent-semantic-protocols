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

end ASPProof.IncrementalCacheAuthority
