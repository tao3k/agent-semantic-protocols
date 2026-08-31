namespace ASPProof.AgentSessionLifecycleProduct

inductive ServerHealth where
  | unobserved
  | ready
  | repairing
  | unavailable
  deriving DecidableEq, Repr

structure WorkspaceServer where
  epoch : Nat
  health : ServerHealth
  deriving DecidableEq, Repr

inductive SessionPhase where
  | unobserved
  | declared
  | active
  | interrupted
  | completed
  | failed
  deriving DecidableEq, Repr

structure SessionLifecycle where
  generation : Nat
  phase : SessionPhase
  deriving DecidableEq, Repr

inductive BindingPhase where
  | unobserved
  | unbound
  | fresh
  | stale
  deriving DecidableEq, Repr

inductive PathObservation where
  | unobserved
  | absent
  | present
  deriving DecidableEq, Repr

structure HostBinding where
  generation : Nat
  childId : Nat
  canonicalTarget : Nat
  phase : BindingPhase
  pathObservation : PathObservation
  deriving DecidableEq, Repr

inductive DispatchPhase where
  | unobserved
  | idle
  | claimed
  | running
  | quarantined
  | completed
  | rejected
  deriving DecidableEq, Repr

structure DispatchLifecycle where
  generation : Nat
  dispatchKey : Nat
  phase : DispatchPhase
  deriving DecidableEq, Repr

structure LifecycleProduct where
  server : WorkspaceServer
  session : SessionLifecycle
  binding : HostBinding
  dispatch : DispatchLifecycle
  deriving DecidableEq, Repr

structure HostBindingReceipt where
  generation : Nat
  childId : Nat
  canonicalTarget : Nat
  typedRoleMatches : Bool
  bindingFresh : Bool
  deriving DecidableEq, Repr

inductive RequiredDispatchAction where
  | unavailable
  | spawnAgent
  | followupTask
  deriving DecidableEq, Repr

def receiptMatches
    (session : SessionLifecycle)
    (receipt : HostBindingReceipt) : Prop :=
  receipt.generation = session.generation ∧
    receipt.typedRoleMatches = true ∧
    receipt.bindingFresh = true

def followupTaskAdmitted (state : LifecycleProduct) : Bool :=
  state.binding.pathObservation == .present &&
    state.binding.phase == .fresh &&
    state.binding.generation == state.session.generation

def spawnAgentAdmitted (state : LifecycleProduct) : Bool :=
  state.binding.pathObservation == .absent

def requiredDispatchAction (state : LifecycleProduct) : RequiredDispatchAction :=
  match state.binding.pathObservation with
  | .present => .followupTask
  | .absent => .spawnAgent
  | .unobserved => .unavailable

def durableDispatchAuthorized (state : LifecycleProduct) : Bool :=
  state.session.phase == .active &&
    followupTaskAdmitted state &&
    state.dispatch.generation == state.session.generation

def restartServer (state : LifecycleProduct) : LifecycleProduct :=
  { state with server := { epoch := state.server.epoch + 1, health := .ready } }

def loseServerTransport (state : LifecycleProduct) : LifecycleProduct :=
  { state with server := { state.server with health := .unavailable } }

def observeBindingStale (state : LifecycleProduct) : LifecycleProduct :=
  { state with binding := { state.binding with phase := .stale } }

def quarantineDispatch (state : LifecycleProduct) : LifecycleProduct :=
  { state with dispatch := { state.dispatch with phase := .quarantined } }

def interruptTurn (state : LifecycleProduct) : LifecycleProduct :=
  { state with
    session := { state.session with phase := .interrupted }
    dispatch := { state.dispatch with phase := .completed } }

def completeTurn (state : LifecycleProduct) : LifecycleProduct :=
  { state with
    session := { state.session with phase := .completed }
    dispatch := { state.dispatch with phase := .completed } }

theorem server_restart_preserves_non_server_layers
    (state : LifecycleProduct) :
    (restartServer state).session = state.session ∧
      (restartServer state).binding = state.binding ∧
      (restartServer state).dispatch = state.dispatch := by
  exact ⟨rfl, rfl, rfl⟩

theorem transport_failure_cannot_change_session
    (state : LifecycleProduct) :
    (loseServerTransport state).session = state.session := by
  rfl

theorem stale_binding_does_not_change_session_lifecycle
    (state : LifecycleProduct) :
    (observeBindingStale state).session = state.session := by
  rfl

theorem dispatch_quarantine_does_not_change_session_or_binding
    (state : LifecycleProduct) :
    (quarantineDispatch state).session = state.session ∧
      (quarantineDispatch state).binding = state.binding := by
  exact ⟨rfl, rfl⟩

theorem interrupt_preserves_canonical_agent_path
    (state : LifecycleProduct) :
    (interruptTurn state).binding = state.binding := by
  rfl

theorem completion_preserves_canonical_agent_path
    (state : LifecycleProduct) :
    (completeTurn state).binding = state.binding := by
  rfl

theorem present_path_requires_followup
    (state : LifecycleProduct)
    (present : state.binding.pathObservation = .present) :
    requiredDispatchAction state = .followupTask := by
  simp [requiredDispatchAction, present]

theorem absent_path_requires_spawn
    (state : LifecycleProduct)
    (absent : state.binding.pathObservation = .absent) :
    requiredDispatchAction state = .spawnAgent := by
  simp [requiredDispatchAction, absent]

theorem present_path_rejects_spawn
    (state : LifecycleProduct)
    (present : state.binding.pathObservation = .present) :
    spawnAgentAdmitted state = false := by
  simp [spawnAgentAdmitted, present]

theorem absent_path_rejects_followup
    (state : LifecycleProduct)
    (absent : state.binding.pathObservation = .absent) :
    followupTaskAdmitted state = false := by
  simp [followupTaskAdmitted, absent]

theorem stale_generation_rejects_followup
    (state : LifecycleProduct)
    (stale : state.binding.generation ≠ state.session.generation) :
    followupTaskAdmitted state = false := by
  simp [followupTaskAdmitted, stale]

theorem stale_generation_receipt_cannot_bind
    (session : SessionLifecycle)
    (receipt : HostBindingReceipt)
    (stale : receipt.generation ≠ session.generation) :
    ¬ receiptMatches session receipt := by
  intro hMatches
  exact stale hMatches.1

def receiptMatchesIgnoringHostPolicy
    (session : SessionLifecycle)
    (receipt : HostBindingReceipt)
    (_hostPolicy : Nat) : Prop :=
  receiptMatches session receipt

theorem host_policy_is_not_an_admission_input
    (session : SessionLifecycle)
    (receipt : HostBindingReceipt)
    (leftPolicy rightPolicy : Nat) :
    receiptMatchesIgnoringHostPolicy session receipt leftPolicy =
      receiptMatchesIgnoringHostPolicy session receipt rightPolicy := by
  rfl

end ASPProof.AgentSessionLifecycleProduct
