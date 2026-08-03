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
  | archiveIntentDurable
  | archived
  | retired
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
  | hostTerminated
  | pathReleased
  deriving DecidableEq, Repr

structure HostBinding where
  generation : Nat
  childId : Nat
  canonicalTarget : Nat
  phase : BindingPhase
  terminationReceiptIndexed : Bool
  pathReleaseReceiptIndexed : Bool
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

def receiptMatches
    (session : SessionLifecycle)
    (receipt : HostBindingReceipt) : Prop :=
  receipt.generation = session.generation ∧
    receipt.typedRoleMatches = true ∧
    receipt.bindingFresh = true

def durableDispatchAuthorized (state : LifecycleProduct) : Bool :=
  state.session.phase == .active &&
    state.binding.phase == .fresh &&
    state.binding.generation == state.session.generation &&
    state.dispatch.generation == state.session.generation

def restartServer (state : LifecycleProduct) : LifecycleProduct :=
  { state with
    server := { epoch := state.server.epoch + 1, health := .ready } }

def loseServerTransport (state : LifecycleProduct) : LifecycleProduct :=
  { state with server := { state.server with health := .unavailable } }

def observeBindingStale (state : LifecycleProduct) : LifecycleProduct :=
  { state with binding := { state.binding with phase := .stale } }

def quarantineDispatch (state : LifecycleProduct) : LifecycleProduct :=
  { state with dispatch := { state.dispatch with phase := .quarantined } }

def persistArchiveIntent (state : LifecycleProduct) : LifecycleProduct :=
  { state with session := { state.session with phase := .archiveIntentDurable } }

def indexArchived (state : LifecycleProduct) : LifecycleProduct :=
  { state with session := { state.session with phase := .archived } }

def indexHostTerminated (state : LifecycleProduct) : LifecycleProduct :=
  { state with
    binding := {
      state.binding with
      phase := .hostTerminated
      terminationReceiptIndexed := true } }

def indexPathReleased (state : LifecycleProduct) : LifecycleProduct :=
  { state with
    binding := {
      state.binding with
      phase := .pathReleased
      pathReleaseReceiptIndexed := true } }

def replacementAdmitted (state : LifecycleProduct) (nextGeneration : Nat) : Prop :=
  (state.session.phase = .archived ∨ state.session.phase = .retired) ∧
    state.binding.phase = .pathReleased ∧
    state.binding.terminationReceiptIndexed = true ∧
    state.binding.pathReleaseReceiptIndexed = true ∧
    state.session.generation < nextGeneration

theorem server_restart_preserves_non_server_layers
    (state : LifecycleProduct) :
    (restartServer state).session = state.session ∧
      (restartServer state).binding = state.binding ∧
      (restartServer state).dispatch = state.dispatch := by
  exact ⟨rfl, rfl, rfl⟩

theorem transport_failure_cannot_archive_session
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

theorem archive_intent_does_not_release_canonical_path
    (state : LifecycleProduct) :
    (persistArchiveIntent state).binding = state.binding := by
  rfl

theorem archive_intent_revokes_durable_dispatch
    (state : LifecycleProduct) :
    durableDispatchAuthorized (persistArchiveIntent state) = false := by
  simp [durableDispatchAuthorized, persistArchiveIntent]

theorem replacement_requires_archived_or_retired_session
    (state : LifecycleProduct)
    (nextGeneration : Nat)
    (admitted : replacementAdmitted state nextGeneration) :
    state.session.phase = .archived ∨ state.session.phase = .retired :=
  admitted.1

theorem replacement_requires_path_release
    (state : LifecycleProduct)
    (nextGeneration : Nat)
    (admitted : replacementAdmitted state nextGeneration) :
    state.binding.phase = .pathReleased :=
  admitted.2.1

theorem replacement_requires_host_termination_receipt
    (state : LifecycleProduct)
    (nextGeneration : Nat)
    (admitted : replacementAdmitted state nextGeneration) :
    state.binding.terminationReceiptIndexed = true :=
  admitted.2.2.1

theorem replacement_requires_path_release_receipt
    (state : LifecycleProduct)
    (nextGeneration : Nat)
    (admitted : replacementAdmitted state nextGeneration) :
    state.binding.pathReleaseReceiptIndexed = true :=
  admitted.2.2.2.1

theorem replacement_requires_strictly_newer_generation
    (state : LifecycleProduct)
    (nextGeneration : Nat)
    (admitted : replacementAdmitted state nextGeneration) :
    state.session.generation < nextGeneration :=
  admitted.2.2.2.2

theorem same_generation_replacement_is_rejected
    (state : LifecycleProduct) :
    ¬ replacementAdmitted state state.session.generation := by
  intro admitted
  exact Nat.lt_irrefl state.session.generation admitted.2.2.2.2

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
