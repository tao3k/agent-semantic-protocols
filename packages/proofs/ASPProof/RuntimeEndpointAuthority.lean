namespace ASPProof.RuntimeEndpointAuthority

structure EndpointReadiness where
  controlSocketReady : Bool
  dataSocketReady : Bool
  statusSnapshotReady : Bool
  deriving DecidableEq

def endpointPublicationAuthorized (readiness : EndpointReadiness) : Bool :=
  readiness.controlSocketReady &&
    readiness.dataSocketReady &&
      readiness.statusSnapshotReady

def runtimeHealthy (endpointPublished transportsReady : Bool) : Bool :=
  endpointPublished && transportsReady

inductive PublicTransportBinding
  | uidSharedUnix
  | inheritedConnectedDescriptor
  | loopbackHttp2
  deriving DecidableEq

structure PublishedTransportCapability where
  binding : PublicTransportBinding
  endpointIdentityBound : Bool
  clientFrameProtocolBound : Bool
  callerSandboxReachable : Bool
  deriving DecidableEq

def transportCapabilityUsable (capability : PublishedTransportCapability) : Bool :=
  capability.endpointIdentityBound &&
    capability.clientFrameProtocolBound &&
      capability.callerSandboxReachable

inductive ClientConnectDecision
  | connect (binding : PublicTransportBinding)
  | rejectTransportUnavailable
  deriving DecidableEq

def selectPublishedTransport
    (capability : PublishedTransportCapability) : ClientConnectDecision :=
  if transportCapabilityUsable capability then
    .connect capability.binding
  else
    .rejectTransportUnavailable

theorem optionalTelemetryCannotBlockEndpoint
    (readiness : EndpointReadiness)
    (_telemetryReady : Bool) :
    endpointPublicationAuthorized readiness =
      (readiness.controlSocketReady &&
        readiness.dataSocketReady &&
          readiness.statusSnapshotReady) := by
  rfl

theorem missingEndpointCannotBeHealthy (transportsReady : Bool) :
    runtimeHealthy false transportsReady = false := by
  simp [runtimeHealthy]

theorem endpointPublicationRequiresBothSockets
    (statusSnapshotReady : Bool) :
    endpointPublicationAuthorized
        { controlSocketReady := false
          dataSocketReady := true
          statusSnapshotReady } = false := by
  simp [endpointPublicationAuthorized]

theorem endpointPublicationRequiresStatusSnapshot
    (controlSocketReady dataSocketReady : Bool) :
    endpointPublicationAuthorized
        { controlSocketReady
          dataSocketReady
          statusSnapshotReady := false } = false := by
  simp [endpointPublicationAuthorized]

theorem unreachablePublishedSocketIsNotAClientCapability
    (binding : PublicTransportBinding)
    (endpointIdentityBound clientFrameProtocolBound : Bool) :
    selectPublishedTransport
        { binding
          endpointIdentityBound
          clientFrameProtocolBound
          callerSandboxReachable := false } =
      .rejectTransportUnavailable := by
  simp [selectPublishedTransport, transportCapabilityUsable]

theorem clientConnectRequiresIdentityProtocolAndReachability
    (capability : PublishedTransportCapability)
    (h : selectPublishedTransport capability = .connect capability.binding) :
    capability.endpointIdentityBound = true ∧
      capability.clientFrameProtocolBound = true ∧
        capability.callerSandboxReachable = true := by
  simp [selectPublishedTransport, transportCapabilityUsable] at h
  exact h

theorem transportSelectionHasNoFallbackChain
    (capability : PublishedTransportCapability) :
    selectPublishedTransport capability = .connect capability.binding ∨
      selectPublishedTransport capability = .rejectTransportUnavailable := by
  simp [selectPublishedTransport]

inductive ClientBootstrapAuthority
  | hostSupervisor
  | sandboxClient
  deriving DecidableEq

inductive BootstrapAdmission
  | observeOrRecoverRuntime
  | useInheritedCapability
  | rejectTransportUnavailable
  deriving DecidableEq

def admitClientBootstrap
    (authority : ClientBootstrapAuthority)
    (capability : PublishedTransportCapability) : BootstrapAdmission :=
  match authority with
  | .hostSupervisor => .observeOrRecoverRuntime
  | .sandboxClient =>
      if capability.binding = .inheritedConnectedDescriptor &&
          transportCapabilityUsable capability then
        .useInheritedCapability
      else
        .rejectTransportUnavailable

theorem sandboxInheritedCapabilitySkipsPathBootstrap
    (capability : PublishedTransportCapability)
    (binding : capability.binding = .inheritedConnectedDescriptor)
    (usable : transportCapabilityUsable capability = true) :
    admitClientBootstrap .sandboxClient capability = .useInheritedCapability := by
  simp [admitClientBootstrap, binding, usable]

theorem sandboxClientNeverOwnsRuntimeRecovery
    (capability : PublishedTransportCapability) :
    admitClientBootstrap .sandboxClient capability ≠ .observeOrRecoverRuntime := by
  change
    (if capability.binding = .inheritedConnectedDescriptor &&
          transportCapabilityUsable capability then
        BootstrapAdmission.useInheritedCapability
      else
        BootstrapAdmission.rejectTransportUnavailable) ≠
      BootstrapAdmission.observeOrRecoverRuntime
  split <;> decide

theorem sandboxWithoutUsableCapabilityFailsBeforeSocketIo
    (capability : PublishedTransportCapability)
    (unusable : transportCapabilityUsable capability = false) :
    admitClientBootstrap .sandboxClient capability = .rejectTransportUnavailable := by
  simp [admitClientBootstrap, unusable]

def publicStartSucceeds (endpointPublished endpointHealthy : Bool) : Bool :=
  endpointPublished && endpointHealthy

def endpointVisibleDuringReplacement
    (previousHealthyEndpoint replacementCommitted : Bool) : Bool :=
  if replacementCommitted then true else previousHealthyEndpoint

inductive ReconcileOwnership
  | unownedDeferred
  | supervisorOwned
  deriving DecidableEq

def binaryPublicationAccepted : ReconcileOwnership → Bool
  | .unownedDeferred => false
  | .supervisorOwned => true

theorem ownerSpawnIsNotTerminalStartSuccess :
    publicStartSucceeds false false = false := by
  simp [publicStartSucceeds]

theorem missingEndpointRejectsPublicStartSuccess (endpointHealthy : Bool) :
    publicStartSucceeds false endpointHealthy = false := by
  simp [publicStartSucceeds]

theorem previousEndpointSurvivesUncommittedReplacement :
    endpointVisibleDuringReplacement true false = true := by
  simp [endpointVisibleDuringReplacement]

theorem unownedDeferredReconciliationIsRejected :
    binaryPublicationAccepted .unownedDeferred = false := by
  rfl

theorem supervisorOwnedReconciliationIsAccepted :
    binaryPublicationAccepted .supervisorOwned = true := by
  rfl

end ASPProof.RuntimeEndpointAuthority
