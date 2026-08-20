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
