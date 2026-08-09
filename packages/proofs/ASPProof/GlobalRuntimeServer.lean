namespace ASPProof.GlobalRuntimeServer

abbrev StateHome := String
abbrev ProjectId := String

/-- The public lifecycle of the one global server owned by an ASP State Home. -/
inductive LifecycleState where
  | stopped
  | starting
  | ready
  | stopping
  | failed
  deriving DecidableEq, Repr

/-- Failures remain typed at the model boundary instead of being encoded as readiness. -/
inductive FailureKind where
  | startup
  | endpointBind
  | checkpoint
  | provider (projectId : ProjectId)
  deriving DecidableEq, Repr

/-- Project and workspace identifiers are deliberately absent from server identity. -/
structure ServerIdentity where
  stateHome : StateHome
  deriving DecidableEq, Repr

def canonicalIdentity (stateHome : StateHome) : ServerIdentity :=
  { stateHome }

def identityForRequest (stateHome : StateHome) (_projectId : ProjectId) :
    ServerIdentity :=
  canonicalIdentity stateHome

theorem identity_unique_per_state_home
    (left right : ServerIdentity)
    (sameHome : left.stateHome = right.stateHome) :
    left = right := by
  cases left
  cases right
  simp_all

theorem project_id_cannot_create_server_identity
    (stateHome : StateHome) (leftProject rightProject : ProjectId) :
    identityForRequest stateHome leftProject =
      identityForRequest stateHome rightProject := by
  rfl

/-- `ensure` is a convergent lifecycle request, not a second identity allocator. -/
def ensure : LifecycleState → LifecycleState
  | .stopped | .failed => .starting
  | .starting => .starting
  | .ready => .ready
  | .stopping => .stopping

theorem ensure_is_idempotent (state : LifecycleState) :
    ensure (ensure state) = ensure state := by
  cases state <;> rfl

theorem concurrent_ensure_returns_one_identity
    (stateHome : StateHome) (leftProject rightProject : ProjectId) :
    identityForRequest stateHome leftProject =
      identityForRequest stateHome rightProject := by
  rfl

structure ServerSnapshot where
  identity : ServerIdentity
  lifecycle : LifecycleState
  accepting : Bool
  activeRequests : Nat
  checkpointCommitted : Bool
  failure : Option FailureKind
  deriving DecidableEq, Repr

def beginStop (snapshot : ServerSnapshot) : ServerSnapshot :=
  { snapshot with lifecycle := .stopping, accepting := false }

def requestAdmitted (snapshot : ServerSnapshot) : Prop :=
  snapshot.lifecycle = .ready ∧ snapshot.accepting = true

theorem begin_stop_rejects_new_requests (snapshot : ServerSnapshot) :
    ¬ requestAdmitted (beginStop snapshot) := by
  simp [requestAdmitted, beginStop]

/-- The final stop transition is gated by request drain and checkpoint commit. -/
def finishStop (snapshot : ServerSnapshot) : ServerSnapshot :=
  if snapshot.lifecycle = .stopping ∧
      snapshot.activeRequests = 0 ∧
      snapshot.checkpointCommitted = true then
    { snapshot with lifecycle := .stopped }
  else
    snapshot

theorem completed_stop_requires_drain_and_checkpoint
    (snapshot : ServerSnapshot)
    (wasStopping : snapshot.lifecycle = .stopping)
    (completed : (finishStop snapshot).lifecycle = .stopped) :
    snapshot.activeRequests = 0 ∧ snapshot.checkpointCommitted = true := by
  by_cases drained : snapshot.activeRequests = 0 ∧
      snapshot.checkpointCommitted = true
  · exact drained
  · have guardFalse : ¬ (snapshot.lifecycle = .stopping ∧
        snapshot.activeRequests = 0 ∧ snapshot.checkpointCommitted = true) := by
      intro allowed
      exact drained allowed.2
    have unchanged : finishStop snapshot = snapshot := by
      unfold finishStop
      rw [if_neg guardFalse]
    rw [unchanged, wasStopping] at completed
    cases completed

structure RestartAssumptions where
  drainCompletes : Bool
  checkpointSucceeds : Bool
  startupSucceeds : Bool
  deriving DecidableEq, Repr

/-- A finite abstraction of restart after stop, checkpoint, and startup phases. -/
def restartFinalState (assumptions : RestartAssumptions) : LifecycleState :=
  if assumptions.drainCompletes = true ∧
      assumptions.checkpointSucceeds = true ∧
      assumptions.startupSucceeds = true then
    .ready
  else
    .failed

theorem restart_converges_under_explicit_assumptions
    (assumptions : RestartAssumptions)
    (drains : assumptions.drainCompletes = true)
    (checkpoints : assumptions.checkpointSucceeds = true)
    (starts : assumptions.startupSucceeds = true) :
    restartFinalState assumptions = .ready := by
  simp [restartFinalState, drains, checkpoints, starts]

theorem restart_is_not_unconditionally_claimed :
    restartFinalState ⟨false, true, true⟩ = .failed := by
  rfl

def failStartup (snapshot : ServerSnapshot) (reason : FailureKind) :
    ServerSnapshot :=
  { snapshot with
      lifecycle := .failed
      accepting := false
      failure := some reason }

theorem startup_failure_is_typed
    (snapshot : ServerSnapshot) (reason : FailureKind) :
    (failStartup snapshot reason).lifecycle = .failed ∧
      (failStartup snapshot reason).failure = some reason ∧
      (failStartup snapshot reason).accepting = false := by
  simp [failStartup]

structure Request where
  requestId : Nat
  projectId : ProjectId
  deriving DecidableEq, Repr

structure DispatchState where
  queues : ProjectId → List Nat

def dispatch (state : DispatchState) (request : Request) : DispatchState :=
  { queues := fun projectId =>
      if projectId = request.projectId then
        state.queues projectId ++ [request.requestId]
      else
        state.queues projectId }

theorem dispatch_updates_only_the_target_project
    (state : DispatchState) (request : Request) (otherProject : ProjectId)
    (different : otherProject ≠ request.projectId) :
    (dispatch state request).queues otherProject = state.queues otherProject := by
  simp [dispatch, different]

theorem dispatch_appends_to_the_target_project
    (state : DispatchState) (request : Request) :
    (dispatch state request).queues request.projectId =
      state.queues request.projectId ++ [request.requestId] := by
  simp [dispatch]

inductive ProviderState where
  | healthy
  | failed (reason : FailureKind)
  deriving DecidableEq, Repr

structure RuntimeState where
  server : ServerSnapshot
  providers : ProjectId → ProviderState

def markProviderFailed
    (runtime : RuntimeState) (projectId : ProjectId) (reason : FailureKind) :
    RuntimeState :=
  { runtime with
      providers := fun candidate =>
        if candidate = projectId then .failed reason else runtime.providers candidate }

theorem provider_failure_preserves_global_server_lifecycle
    (runtime : RuntimeState) (projectId : ProjectId) (reason : FailureKind) :
    (markProviderFailed runtime projectId reason).server.lifecycle =
      runtime.server.lifecycle := by
  rfl

theorem provider_failure_isolated_from_other_projects
    (runtime : RuntimeState) (failedProject otherProject : ProjectId)
    (reason : FailureKind) (different : otherProject ≠ failedProject) :
    (markProviderFailed runtime failedProject reason).providers otherProject =
      runtime.providers otherProject := by
  simp [markProviderFailed, different]

/-- Runtime reconciliation carries the installed hook digest through unchanged. -/
structure ControlPlane where
  installedHookDigest : String
  server : ServerSnapshot

def ensureControlPlane (control : ControlPlane) : ControlPlane :=
  { control with
      server := { control.server with lifecycle := ensure control.server.lifecycle } }

theorem runtime_ensure_preserves_fixed_hook_contract (control : ControlPlane) :
    (ensureControlPlane control).installedHookDigest = control.installedHookDigest := by
  rfl

inductive HealthcheckDecision where
  | observeHealthy
  | ensureSupervisor
  deriving DecidableEq, Repr

/-- Historical spawn receipts are diagnostics only. Endpoint health is the
authoritative input to the Global healthcheck reconciliation decision. -/
def healthcheckDecision
    (endpointHealthy : Bool) (_spawnReceiptPresent : Bool) : HealthcheckDecision :=
  if endpointHealthy then .observeHealthy else .ensureSupervisor

theorem absent_endpoint_requires_supervisor_even_with_stale_spawn_receipt :
    healthcheckDecision false true = .ensureSupervisor := by
  rfl

theorem absent_endpoint_requires_supervisor_without_spawn_receipt :
    healthcheckDecision false false = .ensureSupervisor := by
  rfl

theorem healthy_endpoint_avoids_redundant_supervisor_reconcile
    (spawnReceiptPresent : Bool) :
    healthcheckDecision true spawnReceiptPresent = .observeHealthy := by
  cases spawnReceiptPresent <;> rfl

/-- Endpoint hints are discovery inputs; only a fully authenticated handshake
admits the resident server as live. -/
structure SocketObservation where
  endpointHintPresent : Bool
  socketFilePresent : Bool
  connected : Bool
  authenticated : Bool
  protocolMatches : Bool
  identityMatches : Bool
  deriving DecidableEq, Repr

def admitsSocketLiveness (observation : SocketObservation) : Bool :=
  observation.connected &&
    observation.authenticated &&
    observation.protocolMatches &&
    observation.identityMatches

theorem endpoint_hint_cannot_establish_ready :
    admitsSocketLiveness ⟨true, false, false, false, false, false⟩ = false := by
  rfl

theorem socket_file_existence_cannot_establish_ready :
    admitsSocketLiveness ⟨true, true, false, false, false, false⟩ = false := by
  rfl

theorem authenticated_matching_handshake_admits_liveness :
    admitsSocketLiveness ⟨true, true, true, true, true, true⟩ = true := by
  rfl

theorem liveness_requires_authenticated_matching_handshake
    (observation : SocketObservation)
    (live : admitsSocketLiveness observation = true) :
    observation.connected = true ∧
      observation.authenticated = true ∧
      observation.protocolMatches = true ∧
      observation.identityMatches = true := by
  have evidence := live
  simp [admitsSocketLiveness] at evidence
  rcases evidence with ⟨⟨⟨connected, authenticated⟩, protocol⟩, identity⟩
  exact ⟨connected, authenticated, protocol, identity⟩

inductive SocketProbeOutcome where
  | missing
  | refused
  | handshake (observation : SocketObservation)
  deriving DecidableEq, Repr

inductive SocketRecoveryDecision where
  | noMutation
  | elect
  | spawn
  | cleanup
  deriving DecidableEq, Repr

/-- Election is permitted only after transport absence/refusal. Handshake
mismatch is evidence about an extant peer and therefore fails closed. -/
def socketRecoveryDecision
    (operatorStop : Bool) (outcome : SocketProbeOutcome) :
    SocketRecoveryDecision :=
  if operatorStop then
    .noMutation
  else
    match outcome with
    | .missing | .refused => .elect
    | .handshake _ => .noMutation

theorem missing_socket_allows_election :
    socketRecoveryDecision false .missing = .elect := by
  rfl

theorem refused_socket_allows_election :
    socketRecoveryDecision false .refused = .elect := by
  rfl

theorem protocol_mismatch_fails_closed_without_spawn_or_cleanup
    (hint file connected authenticated identity : Bool) :
    socketRecoveryDecision false
      (.handshake ⟨hint, file, connected, authenticated, false, identity⟩) =
      .noMutation := by
  rfl

theorem identity_mismatch_fails_closed_without_spawn_or_cleanup
    (hint file connected authenticated protocol : Bool) :
    socketRecoveryDecision false
      (.handshake ⟨hint, file, connected, authenticated, protocol, false⟩) =
      .noMutation := by
  rfl

theorem operator_stop_has_precedence (outcome : SocketProbeOutcome) :
    socketRecoveryDecision true outcome = .noMutation := by
  rfl

structure SecureHandshake where
  socket : SocketObservation
  kernelPeerUidMatches : Bool
  nonceOwnerEpochMatches : Bool
  nonceFresh : Bool
  deriving DecidableEq, Repr

def admitsSecureHandshake (handshake : SecureHandshake) : Bool :=
  admitsSocketLiveness handshake.socket &&
    handshake.kernelPeerUidMatches &&
    handshake.nonceOwnerEpochMatches &&
    handshake.nonceFresh

theorem secure_admission_requires_kernel_peer_uid_match
    (handshake : SecureHandshake)
    (admitted : admitsSecureHandshake handshake = true) :
    handshake.kernelPeerUidMatches = true := by
  have evidence := admitted
  simp [admitsSecureHandshake] at evidence
  exact evidence.1.1.2

inductive NonceDecision where
  | accept
  | rejectNoMutation
  deriving DecidableEq, Repr

structure NonceLedger where
  ownerEpoch : Nat
  used : Nat → Bool
  usedCount : Nat
  capacity : Nat

def decideNonce
    (ledger : NonceLedger) (ownerEpoch nonce : Nat) : NonceDecision :=
  if ownerEpoch != ledger.ownerEpoch then
    .rejectNoMutation
  else if ledger.used nonce then
    .rejectNoMutation
  else if ledger.usedCount < ledger.capacity then
    .accept
  else
    .rejectNoMutation

def recordNonce
    (ledger : NonceLedger) (ownerEpoch nonce : Nat) : NonceLedger :=
  if decideNonce ledger ownerEpoch nonce = .accept then
    { ledger with
        used := fun candidate => candidate = nonce || ledger.used candidate
        usedCount := ledger.usedCount + 1 }
  else
    ledger

theorem accepted_nonce_belongs_to_owner_epoch_and_is_fresh
    (ledger : NonceLedger) (ownerEpoch nonce : Nat)
    (accepted : decideNonce ledger ownerEpoch nonce = .accept) :
    ownerEpoch = ledger.ownerEpoch ∧ ledger.used nonce = false := by
  simp only [decideNonce] at accepted
  split at accepted <;> simp_all
  split at accepted <;> simp_all

theorem repeated_nonce_fails_closed
    (ledger : NonceLedger) (nonce : Nat)
    (used : ledger.used nonce = true) :
    decideNonce ledger ledger.ownerEpoch nonce = .rejectNoMutation := by
  simp [decideNonce, used]

theorem repeated_nonce_causes_no_mutation
    (ledger : NonceLedger) (nonce : Nat)
    (used : ledger.used nonce = true) :
    recordNonce ledger ledger.ownerEpoch nonce = ledger := by
  simp [recordNonce, repeated_nonce_fails_closed ledger nonce used]

theorem accepted_nonce_is_recorded_as_used
    (ledger : NonceLedger) (ownerEpoch nonce : Nat)
    (accepted : decideNonce ledger ownerEpoch nonce = .accept) :
    (recordNonce ledger ownerEpoch nonce).used nonce = true := by
  simp [recordNonce, accepted]

theorem recorded_nonce_replay_fails_closed
    (ledger : NonceLedger) (ownerEpoch nonce : Nat)
    (accepted : decideNonce ledger ownerEpoch nonce = .accept) :
    decideNonce (recordNonce ledger ownerEpoch nonce) ownerEpoch nonce =
      .rejectNoMutation := by
  have ownerMatches :=
    (accepted_nonce_belongs_to_owner_epoch_and_is_fresh
      ledger ownerEpoch nonce accepted).1
  have marked := accepted_nonce_is_recorded_as_used ledger ownerEpoch nonce accepted
  have ledgerEpoch : (recordNonce ledger ownerEpoch nonce).ownerEpoch = ownerEpoch := by
    simp only [recordNonce, accepted, if_true]
    exact ownerMatches.symm
  simpa only [ledgerEpoch] using
    (repeated_nonce_fails_closed
      (recordNonce ledger ownerEpoch nonce) nonce marked)

theorem nonce_capacity_exhaustion_fails_closed_without_eviction
    (ledger : NonceLedger) (nonce : Nat)
    (exhausted : ledger.capacity ≤ ledger.usedCount) :
    decideNonce ledger ledger.ownerEpoch nonce = .rejectNoMutation := by
  by_cases used : ledger.used nonce = true
  · simp [decideNonce, used]
  · simp [decideNonce, used, Nat.not_lt_of_ge exhausted]

theorem nonce_capacity_exhaustion_causes_no_mutation
    (ledger : NonceLedger) (nonce : Nat)
    (exhausted : ledger.capacity ≤ ledger.usedCount) :
    recordNonce ledger ledger.ownerEpoch nonce = ledger := by
  simp [recordNonce,
    nonce_capacity_exhaustion_fails_closed_without_eviction ledger nonce exhausted]

theorem accepted_nonce_preserves_capacity_bound
    (ledger : NonceLedger) (ownerEpoch nonce : Nat)
    (accepted : decideNonce ledger ownerEpoch nonce = .accept) :
    (recordNonce ledger ownerEpoch nonce).usedCount ≤ ledger.capacity := by
  have ownerMatches :=
    (accepted_nonce_belongs_to_owner_epoch_and_is_fresh
      ledger ownerEpoch nonce accepted).1
  subst ownerEpoch
  have strict : ledger.usedCount < ledger.capacity := by
    by_cases capacityAvailable : ledger.usedCount < ledger.capacity
    · exact capacityAvailable
    · have exhausted : ledger.capacity ≤ ledger.usedCount :=
        Nat.le_of_not_gt capacityAvailable
      have rejected :=
        nonce_capacity_exhaustion_fails_closed_without_eviction
          ledger nonce exhausted
      rw [rejected] at accepted
      cases accepted
  simp only [recordNonce, accepted, if_true]
  omega

structure RuntimePathSecurity where
  runtimeDirIsSymlink : Bool
  runtimeDirUidMatches : Bool
  runtimeDirMode : Nat
  socketMode : Nat
  endpointMode : Nat
  deriving DecidableEq, Repr

def runtimePathsSecure (paths : RuntimePathSecurity) : Prop :=
  paths.runtimeDirIsSymlink = false ∧
    paths.runtimeDirUidMatches = true ∧
    paths.runtimeDirMode = 0o700 ∧
    paths.socketMode = 0o600 ∧
    paths.endpointMode = 0o600

theorem secure_runtime_paths_require_current_uid_and_non_symlink
    (paths : RuntimePathSecurity) (secure : runtimePathsSecure paths) :
    paths.runtimeDirIsSymlink = false ∧ paths.runtimeDirUidMatches = true := by
  exact ⟨secure.1, secure.2.1⟩

theorem secure_runtime_paths_require_private_modes
    (paths : RuntimePathSecurity) (secure : runtimePathsSecure paths) :
    paths.runtimeDirMode = 0o700 ∧
      paths.socketMode = 0o600 ∧ paths.endpointMode = 0o600 := by
  exact ⟨secure.2.2.1, secure.2.2.2.1, secure.2.2.2.2⟩

structure ResourceUse where
  frameBytes : Nat
  connections : Nat
  hookWorkers : Nat
  drainingRequests : Nat
  deriving DecidableEq, Repr

structure ResourceBounds where
  maxFrameBytes : Nat
  maxConnections : Nat
  maxHookWorkers : Nat
  maxDrainingRequests : Nat
  deriving DecidableEq, Repr

def withinResourceBounds (use : ResourceUse) (bounds : ResourceBounds) : Bool :=
  decide (use.frameBytes ≤ bounds.maxFrameBytes) &&
    decide (use.connections ≤ bounds.maxConnections) &&
    decide (use.hookWorkers ≤ bounds.maxHookWorkers) &&
    decide (use.drainingRequests ≤ bounds.maxDrainingRequests)

def boundedResourceAdmission
    (use : ResourceUse) (bounds : ResourceBounds) : NonceDecision := by
  exact if withinResourceBounds use bounds then .accept else .rejectNoMutation

theorem bounded_admission_requires_all_resource_bounds
    (use : ResourceUse) (bounds : ResourceBounds)
    (admitted : boundedResourceAdmission use bounds = .accept) :
    use.frameBytes ≤ bounds.maxFrameBytes ∧
      use.connections ≤ bounds.maxConnections ∧
      use.hookWorkers ≤ bounds.maxHookWorkers ∧
      use.drainingRequests ≤ bounds.maxDrainingRequests := by
  have within : withinResourceBounds use bounds = true := by
    by_cases evidence : withinResourceBounds use bounds = true
    · exact evidence
    · have rejected : boundedResourceAdmission use bounds = .rejectNoMutation := by
        simp [boundedResourceAdmission, evidence]
      rw [rejected] at admitted
      cases admitted
  simp [withinResourceBounds] at within
  rcases within with ⟨⟨⟨frame, connections⟩, workers⟩, draining⟩
  exact ⟨frame, connections, workers, draining⟩

theorem oversized_frame_fails_closed
    (use : ResourceUse) (bounds : ResourceBounds)
    (oversized : bounds.maxFrameBytes < use.frameBytes) :
    boundedResourceAdmission use bounds = .rejectNoMutation := by
  simp [boundedResourceAdmission, withinResourceBounds,
    Nat.not_le_of_lt oversized]

theorem connection_exhaustion_fails_closed
    (use : ResourceUse) (bounds : ResourceBounds)
    (exhausted : bounds.maxConnections < use.connections) :
    boundedResourceAdmission use bounds = .rejectNoMutation := by
  simp [boundedResourceAdmission, withinResourceBounds,
    Nat.not_le_of_lt exhausted]

theorem hook_worker_exhaustion_fails_closed
    (use : ResourceUse) (bounds : ResourceBounds)
    (exhausted : bounds.maxHookWorkers < use.hookWorkers) :
    boundedResourceAdmission use bounds = .rejectNoMutation := by
  simp [boundedResourceAdmission, withinResourceBounds,
    Nat.not_le_of_lt exhausted]

theorem drain_bound_exhaustion_fails_closed
    (use : ResourceUse) (bounds : ResourceBounds)
    (exhausted : bounds.maxDrainingRequests < use.drainingRequests) :
    boundedResourceAdmission use bounds = .rejectNoMutation := by
  simp [boundedResourceAdmission, withinResourceBounds,
    Nat.not_le_of_lt exhausted]

inductive LogField where
  | plain (value : String)
  | sensitive (value : String)
  deriving DecidableEq, Repr

def renderLogField : LogField → String
  | .plain value => value
  | .sensitive _ => "[REDACTED]"

theorem secret_log_field_is_redacted (secret : String) :
    renderLogField (.sensitive secret) = "[REDACTED]" := by
  rfl

end ASPProof.GlobalRuntimeServer
