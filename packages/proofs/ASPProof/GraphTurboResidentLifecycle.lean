import ASPProof.SearchRouterInteractiveGraphState

namespace ASPProof

/-!
Executable refinement for the ASP-owned Graph Turbo resident capability.

The Runtime Server and the Graph Turbo capability deliberately have separate
health.  A missing optional artifact must not make the daemon claim that the
capability is serving, while it also must not erase an otherwise healthy daemon.
-/

inductive GraphTurboResidentState where
  | unavailable
  | starting
  | healthy
  | draining
  | failed
  deriving DecidableEq, Repr

structure GraphTurboProcessIdentity where
  runtimeArtifactDigest : String
  executionCommandDigest : String
  processGeneration : Nat
  deriving DecidableEq, Repr

structure GraphTurboLaunchIdentity where
  configuredExecutionLocator : String
  resolvedArtifactDigest : String
  executionCommandDigest : String
  deriving DecidableEq, Repr

structure GraphTurboResidentConfigEnvelope where
  schemaVersion : String
  launchIdentity : GraphTurboLaunchIdentity
  deriving DecidableEq, Repr

structure GraphTurboRuntimeIdentity where
  sessionId : String
  nodeId : String
  snapshotDigest : String
  workspaceGenerationRootDigest : String
  routeId : String
  deriving DecidableEq, Repr

inductive GraphTurboResultAuthority where
  | candidate
  | proved
  | asserted
  deriving DecidableEq, Repr

structure RuntimeAndGraphTurboHealth where
  daemonHealthy : Bool
  graphTurbo : GraphTurboResidentState
  deriving DecidableEq, Repr

def GraphTurboRankAdmitted
    (state : GraphTurboResidentState)
    (requested response : GraphTurboRuntimeIdentity) : Prop :=
  state = .healthy ∧ requested = response

def GraphTurboProcessAdmitted
    (state : GraphTurboResidentState)
    (active requested : GraphTurboProcessIdentity) : Prop :=
  state = .healthy ∧ active = requested

def GraphTurboLaunchAdmitted
    (configured launched : GraphTurboLaunchIdentity) : Prop :=
  configured = launched

def GraphTurboResidentConfigAdmitted
    (config : GraphTurboResidentConfigEnvelope) : Prop :=
  config.schemaVersion = "1"

def GraphTurboEnvironmentlessReconcileAdmitted
    (published : GraphTurboResidentConfigEnvelope)
    (launched : GraphTurboLaunchIdentity) : Prop :=
  GraphTurboResidentConfigAdmitted published ∧
    published.launchIdentity = launched

def GraphTurboProcessStartAdmitted
    (state : GraphTurboResidentState)
    (active requested : GraphTurboProcessIdentity) : Prop :=
  GraphTurboProcessAdmitted state active requested

def GraphTurboMultiplexedCallAdmitted
    (state : GraphTurboResidentState)
    (activeProcess requestedProcess : GraphTurboProcessIdentity)
    (requestedSession responseSession : GraphTurboRuntimeIdentity) : Prop :=
  GraphTurboProcessAdmitted state activeProcess requestedProcess ∧
    requestedSession = responseSession

/- An early handshake-bound draft coupled the process to one semantic
   continuation.  This predicate is intentionally too strong and exists to
   exhibit why the single v1 contract must separate process and call identity. -/
def GraphTurboHandshakeBoundSessionAllowed
    (handshakeSession requestedSession : GraphTurboRuntimeIdentity) : Prop :=
  handshakeSession = requestedSession

def ProcessHandshakeGrantsSemanticAuthority
    (_process : GraphTurboProcessIdentity)
    (_session : GraphTurboRuntimeIdentity) : Prop :=
  False

def GraphTurboResultAccepted (authority : GraphTurboResultAuthority) : Prop :=
  authority = .candidate

theorem missingArtifactLeavesCapabilityUnavailable
    (active requested : GraphTurboRuntimeIdentity) :
    ¬ GraphTurboRankAdmitted .unavailable active requested := by
  simp [GraphTurboRankAdmitted]

theorem missingArtifactDoesNotFalsifyDaemonHealth
    (graphTurbo : GraphTurboResidentState) :
    (RuntimeAndGraphTurboHealth.mk true graphTurbo).daemonHealthy = true := by
  rfl

theorem staleRuntimeIdentityRejectsRank
    (state : GraphTurboResidentState)
    (active requested : GraphTurboRuntimeIdentity)
    (stale : active ≠ requested) :
    ¬ GraphTurboRankAdmitted state active requested := by
  simp [GraphTurboRankAdmitted, stale]

theorem processHandshakeCannotGrantSemanticAuthority
    (process : GraphTurboProcessIdentity)
    (session : GraphTurboRuntimeIdentity) :
    ¬ ProcessHandshakeGrantsSemanticAuthority process session := by
  simp [ProcessHandshakeGrantsSemanticAuthority]

theorem sameArtifactDoesNotAuthorizeExecutionLocatorSubstitution
    (configured launched : GraphTurboLaunchIdentity)
    (_sameArtifact : configured.resolvedArtifactDigest = launched.resolvedArtifactDigest)
    (differentLocator :
      configured.configuredExecutionLocator ≠ launched.configuredExecutionLocator) :
    ¬ GraphTurboLaunchAdmitted configured launched := by
  intro admitted
  have exactIdentity : configured = launched := admitted
  exact differentLocator (congrArg GraphTurboLaunchIdentity.configuredExecutionLocator exactIdentity)

theorem environmentlessReconcilePreservesConfiguredExecutionLocator
    (published : GraphTurboResidentConfigEnvelope)
    (launched : GraphTurboLaunchIdentity)
    (admitted : GraphTurboEnvironmentlessReconcileAdmitted published launched) :
    published.launchIdentity.configuredExecutionLocator =
      launched.configuredExecutionLocator := by
  exact congrArg GraphTurboLaunchIdentity.configuredExecutionLocator admitted.2

theorem unknownResidentConfigVersionFailsClosed
    (config : GraphTurboResidentConfigEnvelope)
    (unknown : config.schemaVersion ≠ "1") :
    ¬ GraphTurboResidentConfigAdmitted config := by
  simpa [GraphTurboResidentConfigAdmitted] using unknown

theorem processStartAdmissionIsWorkspaceGenerationIndependent
    (state : GraphTurboResidentState)
    (active requested : GraphTurboProcessIdentity)
    (_sessionA _sessionB : GraphTurboRuntimeIdentity) :
    GraphTurboProcessStartAdmitted state active requested =
      GraphTurboProcessAdmitted state active requested := by
  rfl

theorem handshakeBoundProcessRejectsDistinctLiveSession
    (handshakeSession requestedSession : GraphTurboRuntimeIdentity)
    (distinct : handshakeSession ≠ requestedSession) :
    ¬ GraphTurboHandshakeBoundSessionAllowed handshakeSession requestedSession := by
  simp [GraphTurboHandshakeBoundSessionAllowed, distinct]

theorem oneProcessAdmitsTwoExactSessions
    (process : GraphTurboProcessIdentity)
    (sessionA sessionB : GraphTurboRuntimeIdentity) :
    GraphTurboMultiplexedCallAdmitted .healthy process process sessionA sessionA ∧
      GraphTurboMultiplexedCallAdmitted .healthy process process sessionB sessionB := by
  simp [GraphTurboMultiplexedCallAdmitted, GraphTurboProcessAdmitted]

theorem crossSessionSubstitutionRejected
    (process : GraphTurboProcessIdentity)
    (sessionA sessionB : GraphTurboRuntimeIdentity)
    (distinct : sessionA ≠ sessionB) :
    ¬ GraphTurboMultiplexedCallAdmitted .healthy process process sessionA sessionB := by
  simp [GraphTurboMultiplexedCallAdmitted, GraphTurboProcessAdmitted, distinct]

theorem provedAuthorityCannotCrossCandidateBoundary :
    ¬ GraphTurboResultAccepted .proved := by
  simp [GraphTurboResultAccepted]

theorem assertedAuthorityCannotCrossCandidateBoundary :
    ¬ GraphTurboResultAccepted .asserted := by
  simp [GraphTurboResultAccepted]

theorem healthyExactIdentityAdmitsCandidateRank
    (identity : GraphTurboRuntimeIdentity) :
    GraphTurboRankAdmitted .healthy identity identity ∧
      GraphTurboResultAccepted .candidate := by
  simp [GraphTurboRankAdmitted, GraphTurboResultAccepted]

end ASPProof
