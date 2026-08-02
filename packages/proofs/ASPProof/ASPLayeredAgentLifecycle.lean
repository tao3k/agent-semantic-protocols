namespace ASPProof.ASPLayeredAgentLifecycle

inductive TurnPhase where
  | idle
  | queued
  | running
  | completed
  | failed
  | interrupted
  deriving DecidableEq, Repr

inductive Visibility where
  | visible
  | hidden
  | notLoaded
  deriving DecidableEq, Repr

structure ServerProcess where
  pid : Nat
  runtimeGeneration : Nat
  artifactDigest : Nat
  ipcReady : Bool
  deriving DecidableEq, Repr

structure AgentSession where
  canonicalPath : Nat
  agentId : Nat
  physicalGeneration : Nat
  profileCurrent : Bool
  bindingDelivered : Bool
  routeable : Bool
  terminatedReceipt : Bool
  pathReleaseReceipt : Bool
  deriving DecidableEq, Repr

structure CodexTurn where
  phase : TurnPhase
  dispatchIntent : Option Nat
  deliveredReceipt : Option Nat
  deriving DecidableEq, Repr

structure LayeredState where
  server : ServerProcess
  session : AgentSession
  turn : CodexTurn
  visibility : Visibility
  deriving DecidableEq, Repr

def hasResidentAuthority (session : AgentSession) : Bool :=
  session.profileCurrent && session.bindingDelivered && session.routeable &&
    !session.pathReleaseReceipt

def replacementAdmitted (session : AgentSession) (nextGeneration : Nat) : Prop :=
  session.terminatedReceipt = true ∧
    session.pathReleaseReceipt = true ∧
    session.physicalGeneration < nextGeneration

def restartServer (state : LayeredState) (nextPid nextArtifact : Nat) : LayeredState :=
  { state with
    server :=
      { pid := nextPid
        runtimeGeneration := state.server.runtimeGeneration + 1
        artifactDigest := nextArtifact
        ipcReady := true } }

def loseServerTransport (state : LayeredState) : LayeredState :=
  { state with server := { state.server with ipcReady := false } }

def observeVisibility (state : LayeredState) (visibility : Visibility) : LayeredState :=
  { state with visibility := visibility }

def completeTurn (state : LayeredState) : LayeredState :=
  { state with turn := { state.turn with phase := .completed } }

def interruptTurn (state : LayeredState) : LayeredState :=
  { state with turn := { state.turn with phase := .interrupted } }

theorem serverRestartPreservesSession
    (state : LayeredState) (nextPid nextArtifact : Nat) :
    (restartServer state nextPid nextArtifact).session = state.session := by
  rfl

theorem serverRestartPreservesResidentAuthority
    (state : LayeredState) (nextPid nextArtifact : Nat) :
    hasResidentAuthority (restartServer state nextPid nextArtifact).session =
      hasResidentAuthority state.session := by
  rfl

theorem runtimeIdentityCannotAuthorizeReplacement
    (state : LayeredState) (nextPid nextArtifact nextGeneration : Nat) :
    replacementAdmitted (restartServer state nextPid nextArtifact).session nextGeneration =
      replacementAdmitted state.session nextGeneration := by
  rfl

theorem visibilityCannotRemoveResidentAuthority
    (state : LayeredState) (visibility : Visibility) :
    hasResidentAuthority (observeVisibility state visibility).session =
      hasResidentAuthority state.session := by
  rfl

theorem transportFailureCannotProveResidentAbsence
    (state : LayeredState) :
    hasResidentAuthority (loseServerTransport state).session =
      hasResidentAuthority state.session := by
  rfl

theorem completionDoesNotReleaseCanonicalPath (state : LayeredState) :
    (completeTurn state).session.pathReleaseReceipt =
      state.session.pathReleaseReceipt := by
  rfl

theorem interruptDoesNotReleaseCanonicalPath (state : LayeredState) :
    (interruptTurn state).session.pathReleaseReceipt =
      state.session.pathReleaseReceipt := by
  rfl

theorem replacementRequiresTermination
    (session : AgentSession) (nextGeneration : Nat)
    (h : replacementAdmitted session nextGeneration) :
    session.terminatedReceipt = true := by
  exact h.1

theorem replacementRequiresPathRelease
    (session : AgentSession) (nextGeneration : Nat)
    (h : replacementAdmitted session nextGeneration) :
    session.pathReleaseReceipt = true := by
  exact h.2.1

theorem sameGenerationReplacementRejected (session : AgentSession) :
    ¬replacementAdmitted session session.physicalGeneration := by
  intro h
  exact Nat.lt_irrefl session.physicalGeneration h.2.2

end ASPProof.ASPLayeredAgentLifecycle
