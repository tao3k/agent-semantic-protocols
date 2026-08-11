namespace ASPProof.HookMemoryInboxDeadlockFreedom

inductive HookEventKind
  | hostLifecycle
  | hostExecution
  | workspaceMutation
  | performanceObservation
  deriving DecidableEq

inductive RuntimeState
  | unavailable
  | healthy
  deriving DecidableEq

inductive HookResult
  | appended
  | typedFailure
  deriving DecidableEq

structure InboxEvent where
  sequence : Nat
  hostIdentity : String
  kind : HookEventKind
  deriving DecidableEq

def hookProject (_runtime : RuntimeState) (event : InboxEvent) : HookResult × InboxEvent :=
  (.appended, event)

theorem hookProjectionDoesNotDependOnRuntime
    (left right : RuntimeState) (event : InboxEvent) :
    hookProject left event = hookProject right event := by
  rfl

theorem unavailableRuntimeCannotBlockAppend (event : InboxEvent) :
    (hookProject .unavailable event).1 = .appended := by
  rfl

theorem everyHookEventKindIsRuntimeIndependent
    (kind : HookEventKind) (identity : String) (left right : RuntimeState) :
    hookProject left { sequence := 1, hostIdentity := identity, kind := kind } =
      hookProject right { sequence := 1, hostIdentity := identity, kind := kind } := by
  rfl

theorem hookProjectionPreservesIdentity
    (runtime : RuntimeState) (event : InboxEvent) :
    (hookProject runtime event).2 = event := by
  rfl

def pendingAfterAppend (result : HookResult) : Bool :=
  match result with
  | .appended => true
  | .typedFailure => false

theorem acknowledgedAppendIsPending (runtime : RuntimeState) (event : InboxEvent) :
    pendingAfterAppend (hookProject runtime event).1 = true := by
  rfl

structure ReconcileReceipt where
  generation : Nat
  materializedIdentity : String
  deriving DecidableEq

def reconcile (event : InboxEvent) : ReconcileReceipt :=
  { generation := 1, materializedIdentity := event.hostIdentity }

theorem reconcileCreatesPositiveGeneration (event : InboxEvent) :
    (reconcile event).generation > 0 := by
  simp [reconcile]

theorem reconcilePreservesHostIdentity (event : InboxEvent) :
    (reconcile event).materializedIdentity = event.hostIdentity := by
  rfl

/-- Agent-session registry is the lifecycle authority. The mmap inbox is only a
recovery log used when direct Runtime delivery is unavailable. -/
inductive LifecycleDelivery
  | registryCommitted
  | recoveryLogAppended
  | typedFailure
  deriving DecidableEq

def deliverHostLifecycle
    (runtime : RuntimeState) (recoveryLogWritable : Bool) : LifecycleDelivery :=
  match runtime with
  | .healthy => .registryCommitted
  | .unavailable =>
      if recoveryLogWritable then .recoveryLogAppended else .typedFailure

theorem healthy_runtime_does_not_depend_on_recovery_log_lock
    (recoveryLogWritable : Bool) :
    deliverHostLifecycle .healthy recoveryLogWritable = .registryCommitted := by
  rfl

inductive RegistryChoiceState
  | live
  | registrationRequired
  | blocked
  deriving DecidableEq

inductive InboxReplayState
  | reconciled
  | lockOpenPermissionDenied
  | corrupt
  deriving DecidableEq

/-- ChoicePlane state is projected only from authoritative registry state;
recovery-log replay is observable diagnostics, never an admission dependency. -/
def choicePlaneState
    (registry : RegistryChoiceState) (_inbox : InboxReplayState) : RegistryChoiceState :=
  registry

theorem inbox_permission_denial_cannot_block_live_choice_plane :
    choicePlaneState .live .lockOpenPermissionDenied = .live := by
  rfl

theorem inbox_permission_denial_cannot_block_registration_choice :
    choicePlaneState .registrationRequired .lockOpenPermissionDenied =
      .registrationRequired := by
  rfl

/-- Counterexample for the removed architecture: coupling ChoicePlane to inbox
replay turns a non-authoritative lock error into a global lifecycle block. -/
def legacyChoicePlaneState
    (registry : RegistryChoiceState) (inbox : InboxReplayState) : RegistryChoiceState :=
  match inbox with
  | .reconciled => registry
  | .lockOpenPermissionDenied | .corrupt => .blocked

theorem legacy_inbox_lock_permission_denial_blocks_registration :
    legacyChoicePlaneState .registrationRequired .lockOpenPermissionDenied = .blocked := by
  rfl

end ASPProof.HookMemoryInboxDeadlockFreedom
