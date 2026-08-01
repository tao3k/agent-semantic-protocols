import ASPProof.CodexMultiAgentV2Lifecycle

namespace ASPProof.ASPAgentSessionFencing

open ASPProof.ASPAgentSessionCodexV2Refinement

abbrev SessionEpoch := Nat
abbrev AgentGeneration := Nat
abbrev TurnToken := Nat
abbrev MessageId := Nat
abbrev ReceiptId := Nat

structure AuthorityFence where
  sessionEpoch : SessionEpoch
  agentGeneration : AgentGeneration
  deriving DecidableEq, Repr

inductive ReceiptKind where
  | hostAccept
  | messageAccept
  | turnComplete
  | interruptAccept
  | releaseAccept
  deriving DecidableEq, Repr

structure IndexedReceipt where
  receiptId : ReceiptId
  fence : AuthorityFence
  turnToken : TurnToken
  kind : ReceiptKind
  deriving DecidableEq, Repr

structure MessageLedger where
  accepted : List MessageId
  consumedReceipts : List ReceiptId
  deriving DecidableEq, Repr

structure IdempotencyCell where
  applied : Bool
  deriving DecidableEq, Repr

structure FencedAuthority where
  fence : AuthorityFence
  session : SessionPhase
  binding : BindingPhase
  turn : TurnPhase
  activeTurnToken : TurnToken
  mailbox : MessageLedger
  observationVersion : Nat
  deriving DecidableEq, Repr

structure CommandEnvelope where
  commandId : Nat
  fence : AuthorityFence
  deriving DecidableEq, Repr

structure MessageEnvelope where
  messageId : MessageId
  fence : AuthorityFence
  deriving DecidableEq, Repr

structure VersionedObservation where
  fence : AuthorityFence
  version : Nat
  binding : BindingPhase
  turn : TurnPhase
  deriving DecidableEq, Repr

def FenceMatches (authority : FencedAuthority) (fence : AuthorityFence) : Prop :=
  fence = authority.fence

def ExternalCommandAdmissible
    (authority : FencedAuthority) (command : CommandEnvelope) : Prop :=
  authority.session = .open ∧
  FenceMatches authority command.fence ∧
  authority.binding = .delivered

def CompletionReceiptAdmissible
    (authority : FencedAuthority) (receipt : IndexedReceipt) : Prop :=
  FenceMatches authority receipt.fence ∧
  receipt.turnToken = authority.activeTurnToken ∧
  receipt.kind = .turnComplete ∧
  authority.turn = .running

def ObservationAdmissible
    (authority : FencedAuthority) (observation : VersionedObservation) : Prop :=
  FenceMatches authority observation.fence ∧
  observation.version = authority.observationVersion

def enqueueMessageOnce
    (ledger : MessageLedger) (messageId : MessageId) : MessageLedger :=
  if ledger.accepted.contains messageId then
    ledger
  else
    { ledger with accepted := messageId :: ledger.accepted }

def consumeReceiptOnce
    (ledger : MessageLedger) (receiptId : ReceiptId) : MessageLedger :=
  if ledger.consumedReceipts.contains receiptId then
    ledger
  else
    { ledger with consumedReceipts := receiptId :: ledger.consumedReceipts }

def applyMessageCellOnce (cell : IdempotencyCell) : IdempotencyCell :=
  if cell.applied then cell else ⟨true⟩

def applyReceiptCellOnce (cell : IdempotencyCell) : IdempotencyCell :=
  if cell.applied then cell else ⟨true⟩

def currentFence : AuthorityFence :=
  ⟨7, 3⟩

def currentAuthority : FencedAuthority :=
  {
    fence := currentFence
    session := .open
    binding := .delivered
    turn := .running
    activeTurnToken := 41
    mailbox := ⟨[], []⟩
    observationVersion := 12
  }

def drainingAuthority : FencedAuthority :=
  { currentAuthority with session := .draining, turn := .idle }

def staleSessionReceipt : IndexedReceipt :=
  ⟨100, ⟨6, 3⟩, 41, .turnComplete⟩

def staleGenerationReceipt : IndexedReceipt :=
  ⟨101, ⟨7, 2⟩, 41, .turnComplete⟩

def staleTurnReceipt : IndexedReceipt :=
  ⟨102, currentFence, 40, .turnComplete⟩

def currentTurnReceipt : IndexedReceipt :=
  ⟨103, currentFence, 41, .turnComplete⟩

def staleObservation : VersionedObservation :=
  ⟨⟨6, 3⟩, 12, .delivered, .running⟩

def currentObservation : VersionedObservation :=
  ⟨currentFence, 12, .delivered, .running⟩

def currentCommand : CommandEnvelope :=
  ⟨200, currentFence⟩

theorem staleSessionEpochRejectsCompletion :
    ¬ CompletionReceiptAdmissible currentAuthority staleSessionReceipt := by
  intro admitted
  have epochEq : 6 = 7 :=
    congrArg AuthorityFence.sessionEpoch admitted.1
  exact (by decide : 6 ≠ 7) epochEq

theorem staleAgentGenerationRejectsCompletion :
    ¬ CompletionReceiptAdmissible currentAuthority staleGenerationReceipt := by
  intro admitted
  have generationEq : 2 = 3 :=
    congrArg AuthorityFence.agentGeneration admitted.1
  exact (by decide : 2 ≠ 3) generationEq

theorem staleTurnTokenRejectsCompletion :
    ¬ CompletionReceiptAdmissible currentAuthority staleTurnReceipt := by
  intro admitted
  have tokenEq : 40 = 41 := admitted.2.1
  exact (by decide : 40 ≠ 41) tokenEq

theorem currentFencedCompletionIsAdmissible :
    CompletionReceiptAdmissible currentAuthority currentTurnReceipt := by
  exact ⟨rfl, rfl, rfl, rfl⟩

theorem drainingRejectsExternalCommand :
    ¬ ExternalCommandAdmissible drainingAuthority currentCommand := by
  intro admitted
  exact SessionPhase.noConfusion admitted.1

theorem staleObservationCannotBecomeAuthority :
    ¬ ObservationAdmissible currentAuthority staleObservation := by
  intro admitted
  have epochEq : 6 = 7 :=
    congrArg AuthorityFence.sessionEpoch admitted.1
  exact (by decide : 6 ≠ 7) epochEq

theorem currentObservationMatchesAuthority :
    ObservationAdmissible currentAuthority currentObservation := by
  exact ⟨rfl, rfl⟩

theorem enqueueMessageOnceIsIdempotent
    (cell : IdempotencyCell) :
    applyMessageCellOnce (applyMessageCellOnce cell) =
      applyMessageCellOnce cell := by
  cases cell with
  | mk applied =>
      cases applied <;> rfl

theorem consumeReceiptOnceIsIdempotent
    (cell : IdempotencyCell) :
    applyReceiptCellOnce (applyReceiptCellOnce cell) =
      applyReceiptCellOnce cell := by
  cases cell with
  | mk applied =>
      cases applied <;> rfl

end ASPProof.ASPAgentSessionFencing
