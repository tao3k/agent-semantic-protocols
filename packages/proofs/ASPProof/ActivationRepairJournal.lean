import ASPProof.ActivationRepairLiveness

namespace ASPProof.ActivationRepairJournal

/-- Durable local state for one canonical repair generation. -/
structure RepairJournal where
  canonicalIdentity : Nat
  previousGeneration : Nat
  activeGeneration : Nat
  authorityRevision : Nat
  entryId : Nat
  entryIdentity : Nat
  entryGeneration : Nat
  entryRevision : Nat
  entryPresent : Bool
  enqueueDurable : Bool
  deliveryAttempts : Nat
  sinkAcknowledged : Bool
  readyEvidenceRevision : Nat
  freshReady : Bool
  rollbackClosed : Bool
  retired : Bool
  deriving Repr, DecidableEq

/-- The sink ledger is external to the local journal. A lost acknowledgement
can therefore leave these two durable systems temporarily divergent. -/
structure RepairSystem where
  journal : RepairJournal
  sinkAcceptedKey : Option Nat
  deriving Repr, DecidableEq

def JournalInvariant (journal : RepairJournal) : Prop :=
  (journal.previousGeneration < journal.activeGeneration →
    journal.entryPresent = true) ∧
  (journal.entryPresent = true →
    journal.entryIdentity = journal.canonicalIdentity ∧
    journal.entryGeneration = journal.activeGeneration ∧
    journal.entryRevision = journal.authorityRevision) ∧
  (journal.enqueueDurable = true → journal.entryPresent = true) ∧
  (journal.sinkAcknowledged = true → journal.enqueueDurable = true) ∧
  (journal.freshReady = true →
    journal.sinkAcknowledged = true ∧
    journal.readyEvidenceRevision = journal.authorityRevision) ∧
  (journal.rollbackClosed = true → journal.freshReady = true) ∧
  (journal.retired = true → journal.rollbackClosed = true)

instance journalInvariantDecidable (journal : RepairJournal) :
    Decidable (JournalInvariant journal) := by
  unfold JournalInvariant
  infer_instance

def emptyJournal : RepairJournal :=
  { canonicalIdentity := 1
    previousGeneration := 1
    activeGeneration := 1
    authorityRevision := 7
    entryId := 0
    entryIdentity := 0
    entryGeneration := 0
    entryRevision := 0
    entryPresent := false
    enqueueDurable := false
    deliveryAttempts := 0
    sinkAcknowledged := false
    readyEvidenceRevision := 0
    freshReady := false
    rollbackClosed := false
    retired := false }

/-- The required atomic linearization: head advancement and journal intent
become durable in the same state transition. -/
def commitWithJournal
    (journal : RepairJournal)
    (candidateGeneration : Nat)
    (entryId : Nat) : RepairJournal :=
  { journal with
    activeGeneration := candidateGeneration
    authorityRevision := journal.authorityRevision + 1
    entryId := entryId
    entryIdentity := journal.canonicalIdentity
    entryGeneration := candidateGeneration
    entryRevision := journal.authorityRevision + 1
    entryPresent := true
    enqueueDurable := false
    deliveryAttempts := 0
    sinkAcknowledged := false
    readyEvidenceRevision := 0
    freshReady := false
    rollbackClosed := false
    retired := false }

/-- Counterexample implementation: the activation head advances without
durable replay intent. -/
def splitCommitWithoutJournal
    (journal : RepairJournal)
    (candidateGeneration : Nat) : RepairJournal :=
  { journal with
    activeGeneration := candidateGeneration
    authorityRevision := journal.authorityRevision + 1
    entryPresent := false
    enqueueDurable := false
    sinkAcknowledged := false
    freshReady := false
    rollbackClosed := false
    retired := false }

def committedJournal : RepairJournal :=
  commitWithJournal emptyJournal 2 2008

def brokenSplitCommit : RepairJournal :=
  splitCommitWithoutJournal emptyJournal 2

def Replayable (journal : RepairJournal) : Prop :=
  journal.entryPresent = true ∧ journal.sinkAcknowledged = false

instance replayableDecidable (journal : RepairJournal) :
    Decidable (Replayable journal) := by
  unfold Replayable
  infer_instance

def enqueuePublication
    (journal : RepairJournal)
    (_present : journal.entryPresent = true) : RepairJournal :=
  { journal with enqueueDurable := true }

def enqueuedJournal : RepairJournal :=
  enqueuePublication committedJournal (by decide)

/-- Pure sink-ledger transition. An accepted key is never overwritten, which
makes replay of the same key idempotent and makes conflicting identity visible. -/
def acceptedKeyAfter
    (journal : RepairJournal)
    (current : Option Nat) : Option Nat :=
  if journal.enqueueDurable = true then
    match current with
    | none => some journal.entryId
    | some accepted => some accepted
  else
    current

/-- Delivery uses the durable journal entry id as the sink idempotency key.
Attempt bookkeeping is local; sink acceptance identity is governed separately
by `acceptedKeyAfter`. -/
def deliverToSink (system : RepairSystem) : RepairSystem :=
  { system with
    journal :=
      { system.journal with
        deliveryAttempts :=
          if system.journal.enqueueDurable = true then
            system.journal.deliveryAttempts + 1
          else
            system.journal.deliveryAttempts }
    sinkAcceptedKey :=
      acceptedKeyAfter system.journal system.sinkAcceptedKey }

def preDeliverySystem : RepairSystem :=
  { journal := enqueuedJournal
    sinkAcceptedKey := none }

/-- Sink acceptance has happened, but the process can crash before recording
the local acknowledgement. -/
def crashAfterSinkBeforeAck : RepairSystem :=
  deliverToSink preDeliverySystem

def CanRecordSinkAck (system : RepairSystem) : Prop :=
  system.journal.enqueueDurable = true ∧
  system.sinkAcceptedKey = some system.journal.entryId

instance canRecordSinkAckDecidable (system : RepairSystem) :
    Decidable (CanRecordSinkAck system) := by
  unfold CanRecordSinkAck
  infer_instance

def recordSinkAck
    (system : RepairSystem)
    (_accepted : CanRecordSinkAck system) : RepairSystem :=
  { system with
    journal := { system.journal with sinkAcknowledged := true } }

def wrongSinkKeySystem : RepairSystem :=
  { journal := enqueuedJournal
    sinkAcceptedKey := some 9999 }

def CanDeriveReady (journal : RepairJournal) : Prop :=
  journal.sinkAcknowledged = true

instance canDeriveReadyDecidable (journal : RepairJournal) :
    Decidable (CanDeriveReady journal) := by
  unfold CanDeriveReady
  infer_instance

def deriveReady
    (journal : RepairJournal)
    (_acknowledged : CanDeriveReady journal) : RepairJournal :=
  { journal with
    readyEvidenceRevision := journal.authorityRevision
    freshReady := true }

def CanCloseRollback (journal : RepairJournal) : Prop :=
  journal.freshReady = true

instance canCloseRollbackDecidable (journal : RepairJournal) :
    Decidable (CanCloseRollback journal) := by
  unfold CanCloseRollback
  infer_instance

def closeRollback
    (journal : RepairJournal)
    (_ready : CanCloseRollback journal) : RepairJournal :=
  { journal with rollbackClosed := true }

def CanRetire (journal : RepairJournal) : Prop :=
  journal.rollbackClosed = true

instance canRetireDecidable (journal : RepairJournal) :
    Decidable (CanRetire journal) := by
  unfold CanRetire
  infer_instance

def retire
    (journal : RepairJournal)
    (_closed : CanRetire journal) : RepairJournal :=
  { journal with retired := true }

theorem empty_journal_is_consistent :
    JournalInvariant emptyJournal := by
  decide

theorem atomic_commit_is_consistent :
    JournalInvariant committedJournal := by
  decide

theorem split_commit_with_enqueue_is_inconsistent :
    ¬ JournalInvariant brokenSplitCommit := by
  decide

theorem crash_before_enqueue_remains_replayable :
    Replayable committedJournal := by
  decide

theorem enqueue_preserves_head_binding
    (present : journal.entryPresent = true) :
    let enqueued := enqueuePublication journal present
    enqueued.activeGeneration = journal.activeGeneration ∧
    enqueued.authorityRevision = journal.authorityRevision ∧
    enqueued.entryId = journal.entryId := by
  exact ⟨rfl, rfl, rfl⟩

theorem duplicate_sink_delivery_is_idempotent_for_acceptance
    (system : RepairSystem) :
    (deliverToSink (deliverToSink system)).sinkAcceptedKey =
      (deliverToSink system).sinkAcceptedKey := by
  change
    acceptedKeyAfter system.journal
        (acceptedKeyAfter system.journal system.sinkAcceptedKey) =
      acceptedKeyAfter system.journal system.sinkAcceptedKey
  unfold acceptedKeyAfter
  by_cases queued : system.journal.enqueueDurable = true
  · rw [if_pos queued]
    cases system.sinkAcceptedKey <;> rw [if_pos queued] <;> rfl
  · rw [if_neg queued]

theorem delivery_does_not_advance_activation_head
    (system : RepairSystem) :
    (deliverToSink system).journal.activeGeneration =
        system.journal.activeGeneration ∧
      (deliverToSink system).journal.authorityRevision =
        system.journal.authorityRevision := by
  exact ⟨rfl, rfl⟩

theorem crash_after_sink_can_record_ack :
    CanRecordSinkAck crashAfterSinkBeforeAck := by
  decide

theorem replay_after_lost_ack_preserves_sink_acceptance :
    (deliverToSink crashAfterSinkBeforeAck).sinkAcceptedKey =
      crashAfterSinkBeforeAck.sinkAcceptedKey := by
  decide

theorem wrong_sink_key_cannot_ack :
    ¬ CanRecordSinkAck wrongSinkKeySystem := by
  decide

theorem commit_without_ack_cannot_derive_ready :
    ¬ CanDeriveReady committedJournal := by
  decide

theorem ready_without_rollback_cannot_retire :
    let acknowledged := recordSinkAck crashAfterSinkBeforeAck (by decide)
    let ready := deriveReady acknowledged.journal (by decide)
    ¬ CanRetire ready := by
  decide

theorem proof_gated_retirement_is_consistent :
    let acknowledged := recordSinkAck crashAfterSinkBeforeAck (by decide)
    let ready := deriveReady acknowledged.journal (by decide)
    let closed := closeRollback ready (by decide)
    let retired := retire closed (by decide)
    JournalInvariant retired := by
  decide

end ASPProof.ActivationRepairJournal
