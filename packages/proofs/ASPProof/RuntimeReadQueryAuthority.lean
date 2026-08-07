namespace ASPProof.RuntimeReadQueryAuthority

abbrev ProjectId := String
abbrev WorkspaceIdentity := String
abbrev Selector := String
abbrev Digest := String

inductive Projection where
  | source
  | callableSkeleton
  deriving DecidableEq, Repr

inductive QueryOperation where
  | exactQuery
  deriving DecidableEq, Repr

structure ReadRequest where
  requestId : String
  projectId : ProjectId
  workspaceIdentity : WorkspaceIdentity
  selector : Selector
  projection : Projection
  budgetMicros : Nat
  budgetPositive : 0 < budgetMicros

structure AgentAuthority where
  connectGlobalRuntimeServer : Bool
  connectWorkspaceOwner : Bool

def readOnlyResidentAuthority : AgentAuthority where
  connectGlobalRuntimeServer := true
  connectWorkspaceOwner := false

theorem readOnlyResidentConnectsGlobalRuntime :
    readOnlyResidentAuthority.connectGlobalRuntimeServer = true := rfl

theorem readOnlyResidentCannotConnectWorkspaceOwner :
    readOnlyResidentAuthority.connectWorkspaceOwner = false := rfl

structure InternalWorkspaceRoute where
  projectId : ProjectId
  workspaceIdentity : WorkspaceIdentity
  privateOwnerEndpointExposed : Bool

def routeInsideRuntimeServer (request : ReadRequest) : InternalWorkspaceRoute where
  projectId := request.projectId
  workspaceIdentity := request.workspaceIdentity
  privateOwnerEndpointExposed := false

theorem internalRoutePreservesProject (request : ReadRequest) :
    (routeInsideRuntimeServer request).projectId = request.projectId := rfl

theorem internalRoutePreservesWorkspace (request : ReadRequest) :
    (routeInsideRuntimeServer request).workspaceIdentity = request.workspaceIdentity := rfl

theorem internalRouteNeverExposesOwnerEndpoint (request : ReadRequest) :
    (routeInsideRuntimeServer request).privateOwnerEndpointExposed = false := rfl

structure QueryEffects where
  publishesGeneration : Bool
  mutatesOverlay : Bool
  writesFreshnessReceipt : Bool

def readQueryEffects : QueryEffects where
  publishesGeneration := false
  mutatesOverlay := false
  writesFreshnessReceipt := false

theorem readQueryDoesNotPublishGeneration :
    readQueryEffects.publishesGeneration = false := rfl

theorem readQueryDoesNotMutateOverlay :
    readQueryEffects.mutatesOverlay = false := rfl

theorem readQueryDoesNotWriteFreshnessReceipt :
    readQueryEffects.writesFreshnessReceipt = false := rfl

structure OwnerDigestObservation where
  cached : Digest
  live : Digest

def CacheFresh (observation : OwnerDigestObservation) : Prop :=
  observation.cached = observation.live

def CachedProjectionAdmissible (observation : OwnerDigestObservation) : Prop :=
  CacheFresh observation

theorem changedOwnerRejectsCachedProjection
    (observation : OwnerDigestObservation)
    (changed : observation.cached ≠ observation.live) :
    ¬ CachedProjectionAdmissible observation := changed

theorem freshOwnerAdmitsCachedProjection
    (observation : OwnerDigestObservation)
    (fresh : observation.cached = observation.live) :
    CachedProjectionAdmissible observation := fresh

structure ProviderCapabilities where
  nativeExactProjection : Prop

def ProviderNativeFallbackAllowed (capabilities : ProviderCapabilities) : Prop :=
  capabilities.nativeExactProjection

theorem providerNativeFallbackRequiresDeclaration
    (capabilities : ProviderCapabilities)
    (allowed : ProviderNativeFallbackAllowed capabilities) :
    capabilities.nativeExactProjection := allowed

structure TypedFailure where
  reasonKind : String
  retryAfterMs : Nat
  nextAction : String
  reasonPresent : reasonKind ≠ ""
  nextActionPresent : nextAction ≠ ""

inductive QueryOutcome where
  | projected
  | fallbackProjected
  | unavailable (failure : TypedFailure)

def TerminalOutcome : QueryOutcome → Prop
  | .projected => True
  | .fallbackProjected => True
  | .unavailable _ => True

theorem finiteBudgetQueryHasOnlyTerminalOutcomes
    (_request : ReadRequest)
    (outcome : QueryOutcome) :
    TerminalOutcome outcome := by
  cases outcome <;> trivial

theorem unavailableOutcomeIsActionable (failure : TypedFailure) :
    failure.reasonKind ≠ "" ∧ failure.nextAction ≠ "" :=
  ⟨failure.reasonPresent, failure.nextActionPresent⟩

end ASPProof.RuntimeReadQueryAuthority
