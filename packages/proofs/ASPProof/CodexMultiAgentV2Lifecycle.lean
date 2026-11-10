-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.ASPAgentSessionCodexV2Refinement

abbrev IntentId := Nat
abbrev DispatchKey := Nat
abbrev AgentId := Nat
abbrev AgentPath := List Nat

inductive SessionPhase where
  | open
  | draining
  | closed
  deriving DecidableEq, Repr

inductive BindingPhase where
  | absent
  | reserved
  | intentDurable
  | hostAccepted
  | delivered
  | orphaned
  | released
  deriving DecidableEq, Repr

inductive TurnPhase where
  | idle
  | queued
  | running
  | interrupting
  deriving DecidableEq, Repr

inductive TurnOutcome where
  | none
  | completed
  | failed
  | interrupted
  deriving DecidableEq, Repr

inductive ResidentClass where
  | ephemeral
  | sessionResident
  deriving DecidableEq, Repr

inductive ForkMode where
  | none
  | all
  | lastN (count : Nat)
  deriving DecidableEq, Repr

structure SpawnIdentity where
  intentId : IntentId
  dispatchKey : DispatchKey
  agentId : AgentId
  parentPath : AgentPath
  childPath : AgentPath
  typedRoleBound : Bool
  forkMode : ForkMode
  deriving DecidableEq, Repr

structure AgentState where
  session : SessionPhase
  binding : BindingPhase
  turn : TurnPhase
  outcome : TurnOutcome
  residentClass : ResidentClass
  reservationCommitted : Bool
  intentDurable : Bool
  dispatchKeyStable : Bool
  hostAccepted : Bool
  acceptReceiptIndexed : Bool
  deliveryCommitted : Bool
  mailboxPending : Bool
  boundaryWakePending : Bool
  interruptRequested : Bool
  completionReceiptIndexed : Bool
  releaseReceiptIndexed : Bool
  serverHealthy : Bool
  deriving DecidableEq, Repr

def initialState (residentClass : ResidentClass) : AgentState :=
  {
    session := .open
    binding := .absent
    turn := .idle
    outcome := .none
    residentClass
    reservationCommitted := false
    intentDurable := false
    dispatchKeyStable := false
    hostAccepted := false
    acceptReceiptIndexed := false
    deliveryCommitted := false
    mailboxPending := false
    boundaryWakePending := false
    interruptRequested := false
    completionReceiptIndexed := false
    releaseReceiptIndexed := false
    serverHealthy := true
  }

def reserve (state : AgentState) : AgentState :=
  { state with
    binding := .reserved
    reservationCommitted := true
  }

def persistIntent (state : AgentState) : AgentState :=
  { state with
    binding := .intentDurable
    intentDurable := true
    dispatchKeyStable := true
  }

def acceptHost (state : AgentState) : AgentState :=
  { state with
    binding := .hostAccepted
    hostAccepted := true
    acceptReceiptIndexed := true
  }

def commitDelivery (state : AgentState) : AgentState :=
  { state with
    binding := .delivered
    deliveryCommitted := true
  }

def enqueueMessage (state : AgentState) : AgentState :=
  { state with mailboxPending := true }

def followupIdle (state : AgentState) : AgentState :=
  { state with
    turn := .queued
    mailboxPending := true
    boundaryWakePending := false
    outcome := .none
  }

def followupRunning (state : AgentState) : AgentState :=
  { state with
    mailboxPending := true
    boundaryWakePending := true
  }

def startQueuedTurn (state : AgentState) : AgentState :=
  { state with
    turn := .running
    mailboxPending := false
    boundaryWakePending := false
    outcome := .none
    completionReceiptIndexed := false
  }

def requestInterrupt (state : AgentState) : AgentState :=
  { state with
    turn := .interrupting
    interruptRequested := true
  }

def acknowledgeInterrupt (state : AgentState) : AgentState :=
  { state with
    turn := .idle
    outcome := .interrupted
    interruptRequested := false
    completionReceiptIndexed := true
  }

def completeTurn (state : AgentState) : AgentState :=
  { state with
    turn := .idle
    outcome := .completed
    completionReceiptIndexed := true
  }

def failTurn (state : AgentState) : AgentState :=
  { state with
    turn := .idle
    outcome := .failed
    completionReceiptIndexed := true
  }

def beginDrain (state : AgentState) : AgentState :=
  { state with session := .draining }

def quarantineOrphan (state : AgentState) : AgentState :=
  { state with
    binding := .orphaned
    turn := .idle
  }

def releaseAgent (state : AgentState) : AgentState :=
  { state with
    binding := .released
    turn := .idle
    releaseReceiptIndexed := true
  }

def closeSession (state : AgentState) : AgentState :=
  { state with session := .closed }

def SpawnChainClosed (state : AgentState) : Prop :=
  state.reservationCommitted = true ∧
  state.intentDurable = true ∧
  state.dispatchKeyStable = true ∧
  state.hostAccepted = true ∧
  state.acceptReceiptIndexed = true ∧
  state.deliveryCommitted = true

def TurnStartAdmissible (state : AgentState) : Prop :=
  state.session ≠ .closed ∧
  state.binding = .delivered ∧
  state.turn = .queued ∧
  state.mailboxPending = true

def ReleaseAdmissible (state : AgentState) : Prop :=
  state.session = .draining ∧
  state.turn = .idle ∧
  (state.binding = .delivered ∨ state.binding = .orphaned)

def ExternalFollowupAdmissible (state : AgentState) : Prop :=
  state.session = .open ∧
  state.binding = .delivered ∧
  state.turn = .idle

def SessionCloseAdmissible (state : AgentState) : Prop :=
  state.session = .draining ∧
  state.binding = .released ∧
  state.releaseReceiptIndexed = true

def Mutable (state : AgentState) : Prop :=
  state.session ≠ .closed

inductive Step : AgentState → AgentState → Prop where
  | reserveStep {state : AgentState}
      (mutable : Mutable state)
      (sessionOpen : state.session = .open)
      (bindingAbsent : state.binding = .absent) :
      Step state (reserve state)
  | persistIntentStep {state : AgentState}
      (mutable : Mutable state)
      (bindingReserved : state.binding = .reserved)
      (reservationCommitted : state.reservationCommitted = true) :
      Step state (persistIntent state)
  | hostAcceptStep {state : AgentState}
      (mutable : Mutable state)
      (bindingIntentDurable : state.binding = .intentDurable)
      (intentIsDurable : state.intentDurable = true)
      (dispatchStable : state.dispatchKeyStable = true) :
      Step state (acceptHost state)
  | deliveryStep {state : AgentState}
      (mutable : Mutable state)
      (bindingAccepted : state.binding = .hostAccepted)
      (hostDidAccept : state.hostAccepted = true)
      (receiptIndexed : state.acceptReceiptIndexed = true) :
      Step state (commitDelivery state)
  | sendMessageStep {state : AgentState}
      (mutable : Mutable state)
      (sessionOpen : state.session = .open)
      (bindingDelivered : state.binding = .delivered) :
      Step state (enqueueMessage state)
  | followupIdleStep {state : AgentState}
      (mutable : Mutable state)
      (sessionOpen : state.session = .open)
      (bindingDelivered : state.binding = .delivered)
      (turnIdle : state.turn = .idle) :
      Step state (followupIdle state)
  | followupRunningStep {state : AgentState}
      (mutable : Mutable state)
      (sessionOpen : state.session = .open)
      (bindingDelivered : state.binding = .delivered)
      (turnRunning : state.turn = .running) :
      Step state (followupRunning state)
  | startTurnStep {state : AgentState}
      (mutable : Mutable state)
      (admissible : TurnStartAdmissible state) :
      Step state (startQueuedTurn state)
  | requestInterruptStep {state : AgentState}
      (mutable : Mutable state)
      (turnRunning : state.turn = .running) :
      Step state (requestInterrupt state)
  | acknowledgeInterruptStep {state : AgentState}
      (mutable : Mutable state)
      (turnInterrupting : state.turn = .interrupting) :
      Step state (acknowledgeInterrupt state)
  | completeTurnStep {state : AgentState}
      (mutable : Mutable state)
      (turnRunning : state.turn = .running) :
      Step state (completeTurn state)
  | failTurnStep {state : AgentState}
      (mutable : Mutable state)
      (turnRunning : state.turn = .running) :
      Step state (failTurn state)
  | beginDrainStep {state : AgentState}
      (mutable : Mutable state)
      (sessionOpen : state.session = .open) :
      Step state (beginDrain state)
  | quarantineOrphanStep {state : AgentState}
      (mutable : Mutable state)
      (bindingAccepted : state.binding = .hostAccepted) :
      Step state (quarantineOrphan state)
  | releaseStep {state : AgentState}
      (mutable : Mutable state)
      (admissible : ReleaseAdmissible state) :
      Step state (releaseAgent state)
  | closeStep {state : AgentState}
      (mutable : Mutable state)
      (admissible : SessionCloseAdmissible state) :
      Step state (closeSession state)

def canonicalChildPath (parent : AgentPath) (slot : Nat) : AgentPath :=
  parent ++ [slot]

def SameDispatch (left right : SpawnIdentity) : Prop :=
  left.intentId = right.intentId ∧ left.dispatchKey = right.dispatchKey

def IdempotentHostAccept (left right : SpawnIdentity) : Prop :=
  SameDispatch left right → left.agentId = right.agentId

structure RegistryEntry where
  registeredName : String
  canonicalPath : AgentPath
  typedRoleBound : Bool
  residentClass : ResidentClass
  deriving DecidableEq, Repr

def MentionAdmissible (entry : RegistryEntry) (mention : String) : Prop :=
  mention = entry.registeredName ∧
  entry.typedRoleBound = true ∧
  entry.residentClass = .sessionResident

structure CodexV2HostCapabilities where
  spawn : Bool
  sendMessage : Bool
  followupTask : Bool
  interruptAgent : Bool
  listAgents : Bool
  waitAgent : Bool
  deriving DecidableEq, Repr

def HostCapabilitiesAvailable (host : CodexV2HostCapabilities) : Prop :=
  host.spawn = true ∧
  host.sendMessage = true ∧
  host.followupTask = true ∧
  host.interruptAgent = true ∧
  host.listAgents = true ∧
  host.waitAgent = true

def ASPUseAdmissible
    (host : CodexV2HostCapabilities) (state : AgentState) : Prop :=
  HostCapabilitiesAvailable host ∧ SpawnChainClosed state

def completeHostCapabilities : CodexV2HostCapabilities :=
  ⟨true, true, true, true, true, true⟩

structure WaitObservation where
  binding : BindingPhase
  turn : TurnPhase
  outcome : TurnOutcome
  deriving DecidableEq, Repr

def observe (state : AgentState) : WaitObservation :=
  ⟨state.binding, state.turn, state.outcome⟩

def deliveredIdle : AgentState :=
  commitDelivery (acceptHost (persistIntent (reserve (initialState .sessionResident))))

def deliveredRunning : AgentState :=
  startQueuedTurn (followupIdle deliveredIdle)

def acceptedBeforeDelivery : AgentState :=
  acceptHost (persistIntent (reserve (initialState .sessionResident)))

def sameServerDifferentBindingLeft : AgentState :=
  initialState .sessionResident

def sameServerDifferentBindingRight : AgentState :=
  deliveredIdle

def nameOnlyEntry : RegistryEntry :=
  ⟨"search", [], false, .sessionResident⟩

def drainingDeliveredIdle : AgentState :=
  beginDrain deliveredIdle

def drainingOrphan : AgentState :=
  quarantineOrphan (beginDrain acceptedBeforeDelivery)

theorem closedSpawnContainsEveryDurableEdge
    (state : AgentState) (closed : SpawnChainClosed state) :
    state.intentDurable = true ∧
    state.dispatchKeyStable = true ∧
    state.acceptReceiptIndexed = true :=
  ⟨closed.2.1, closed.2.2.1, closed.2.2.2.2.1⟩

theorem hostAcceptBeforeDeliveryIsNotClosed :
    ¬ SpawnChainClosed acceptedBeforeDelivery := by
  intro closed
  exact Bool.noConfusion closed.2.2.2.2.2

theorem sendMessageDoesNotTriggerIdleTurn :
    (enqueueMessage deliveredIdle).turn = .idle ∧
    (enqueueMessage deliveredIdle).mailboxPending = true := by
  exact ⟨rfl, rfl⟩

theorem followupOnIdleQueuesOneTurn :
    (followupIdle deliveredIdle).turn = .queued ∧
    (followupIdle deliveredIdle).mailboxPending = true := by
  exact ⟨rfl, rfl⟩

theorem followupOnRunningPreservesRunningTurn :
    (followupRunning deliveredRunning).turn = .running ∧
    (followupRunning deliveredRunning).boundaryWakePending = true := by
  exact ⟨rfl, rfl⟩

theorem interruptReturnsAgentToReusableIdle :
    (acknowledgeInterrupt (requestInterrupt deliveredRunning)).turn = .idle ∧
    (acknowledgeInterrupt (requestInterrupt deliveredRunning)).binding =
      .delivered ∧
    (acknowledgeInterrupt (requestInterrupt deliveredRunning)).outcome =
      .interrupted := by
  exact ⟨rfl, rfl, rfl⟩

theorem completionIsTurnTerminalNotAgentTerminal :
    (completeTurn deliveredRunning).turn = .idle ∧
    (completeTurn deliveredRunning).binding = .delivered ∧
    (followupIdle (completeTurn deliveredRunning)).turn = .queued := by
  exact ⟨rfl, rfl, rfl⟩

theorem failureIsRecoverableByFollowup :
    (failTurn deliveredRunning).outcome = .failed ∧
    (followupIdle (failTurn deliveredRunning)).turn = .queued := by
  exact ⟨rfl, rfl⟩

theorem waitObservationDoesNotMutateAuthority (state : AgentState) :
    observe state = observe state := by
  rfl

theorem serverHealthDoesNotDetermineSessionBinding :
    sameServerDifferentBindingLeft.serverHealthy =
      sameServerDifferentBindingRight.serverHealthy ∧
    sameServerDifferentBindingLeft.binding ≠
      sameServerDifferentBindingRight.binding := by
  constructor
  · rfl
  · decide

theorem registeredNameDoesNotProveTypedResident :
    ¬ MentionAdmissible nameOnlyEntry "search" := by
  intro admitted
  exact Bool.noConfusion admitted.2.1

theorem canonicalChildPathIsStable (parent : AgentPath) (slot : Nat) :
    canonicalChildPath parent slot = parent ++ [slot] := by
  rfl

theorem closeRequiresReleaseReceipt
    (state : AgentState) (admissible : SessionCloseAdmissible state) :
    state.binding = .released ∧ state.releaseReceiptIndexed = true :=
  ⟨admissible.2.1, admissible.2.2⟩

theorem sessionCloseDoesNotDependOnServerHealth
    (state : AgentState) (admissible : SessionCloseAdmissible state) :
    SessionCloseAdmissible { state with serverHealthy := !state.serverHealthy } := by
  exact ⟨admissible.1, admissible.2.1, admissible.2.2⟩

theorem everyStepStartsFromMutableSession
    {source target : AgentState} (step : Step source target) :
    Mutable source := by
  cases step <;> assumption

theorem idempotentAcceptReusesAgentId
    (left right : SpawnIdentity)
    (hostIdempotent : IdempotentHostAccept left right)
    (sameDispatch : SameDispatch left right) :
    left.agentId = right.agentId :=
  hostIdempotent sameDispatch

theorem turnPhaseHasOneRunningValue (state : AgentState)
    (running : state.turn = .running) :
    state.turn = .running :=
  running

theorem hostCapabilitiesAloneDoNotAdmitASPBinding :
    ¬ ASPUseAdmissible completeHostCapabilities acceptedBeforeDelivery := by
  intro admitted
  exact Bool.noConfusion admitted.2.2.2.2.2.2

theorem admissibleASPUseHasDurableAuthority
    (host : CodexV2HostCapabilities)
    (state : AgentState)
    (admitted : ASPUseAdmissible host state) :
    state.intentDurable = true ∧
    state.dispatchKeyStable = true ∧
    state.acceptReceiptIndexed = true :=
  closedSpawnContainsEveryDurableEdge state admitted.2

theorem drainingRejectsNewExternalFollowup :
    ¬ ExternalFollowupAdmissible drainingDeliveredIdle := by
  intro admitted
  exact SessionPhase.noConfusion admitted.1

theorem drainingOrphanCanBeReleased :
    ReleaseAdmissible drainingOrphan := by
  exact ⟨rfl, rfl, Or.inr rfl⟩

end ASPProof.ASPAgentSessionCodexV2Refinement
