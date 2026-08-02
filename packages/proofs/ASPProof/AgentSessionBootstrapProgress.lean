namespace ASPProof.AgentSessionBootstrapProgress

inductive TerminalResult where
  | ready
  | waitingForHost
  | failed
  | absent
  deriving DecidableEq, Repr

structure Observation where
  exitCode : Nat
  terminal : TerminalResult
  registryEntry : Bool
  routable : Bool
  commandRequested : Bool
  commandReceipt : Bool
  requestIdentityMatches : Bool
  directOpenAttempted : Bool
  deriving DecidableEq, Repr

def readyEvidence (observation : Observation) : Prop :=
  observation.terminal = .ready ∧
    observation.registryEntry = true ∧
    observation.routable = true ∧
    observation.directOpenAttempted = false ∧
    (observation.commandRequested = true →
      observation.commandReceipt = true ∧
      observation.requestIdentityMatches = true)

def successfulExit (observation : Observation) : Prop :=
  observation.exitCode = 0 ∧
    (readyEvidence observation ∨ observation.terminal = .waitingForHost)

def emptySuccessCounterexample : Observation :=
  { exitCode := 0
    terminal := .absent
    registryEntry := false
    routable := false
    commandRequested := true
    commandReceipt := false
    requestIdentityMatches := false
    directOpenAttempted := true }

theorem processZeroDoesNotProveBootstrap :
    emptySuccessCounterexample.exitCode = 0 := by
  rfl

theorem emptySuccessIsRejected :
    ¬ successfulExit emptySuccessCounterexample := by
  simp [successfulExit, readyEvidence, emptySuccessCounterexample]

theorem readyRequiresRegistryAndRoute
    (observation : Observation)
    (ready : readyEvidence observation) :
    observation.registryEntry = true ∧ observation.routable = true := by
  exact ⟨ready.2.1, ready.2.2.1⟩

theorem readyRejectsDirectOpen
    (observation : Observation)
    (ready : readyEvidence observation) :
    observation.directOpenAttempted = false := by
  exact ready.2.2.2.1

theorem requestedCommandRequiresMatchingReceipt
    (observation : Observation)
    (ready : readyEvidence observation)
    (requested : observation.commandRequested = true) :
    observation.commandReceipt = true ∧
      observation.requestIdentityMatches = true := by
  exact ready.2.2.2.2 requested

end ASPProof.AgentSessionBootstrapProgress
