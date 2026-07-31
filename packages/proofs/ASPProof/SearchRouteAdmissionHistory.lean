import ASPProof.SearchRouteAdmissionLedger

namespace SearchRouteAdmissionHistory

open SearchRouteAdmissionLedger

structure AdmissionRecord (Identity : Type) where
  baselineReceiptIdentity : Identity
  extendedReceiptIdentity : Identity
  deriving DecidableEq

structure CommitmentLedger (Identity : Type) where
  consumedReceipt : Identity → Prop
  committedRecord : Identity → Option (AdmissionRecord Identity)

def CanRecordCommit
    {Identity : Type}
    (ledger : CommitmentLedger Identity)
    (runIdentity : Identity)
    (record : AdmissionRecord Identity) : Prop :=
  record.baselineReceiptIdentity ≠ record.extendedReceiptIdentity
    ∧ ledger.committedRecord runIdentity = none
    ∧ ¬ledger.consumedReceipt record.baselineReceiptIdentity
    ∧ ¬ledger.consumedReceipt record.extendedReceiptIdentity

def AtomicRecordCommit
    {Identity : Type}
    [DecidableEq Identity]
    (before after : CommitmentLedger Identity)
    (runIdentity : Identity)
    (record : AdmissionRecord Identity) : Prop :=
  CanRecordCommit before runIdentity record
    ∧ (∀ identity,
      after.consumedReceipt identity ↔
        before.consumedReceipt identity
          ∨ identity = record.baselineReceiptIdentity
          ∨ identity = record.extendedReceiptIdentity)
    ∧ (∀ identity,
      after.committedRecord identity =
        if identity = runIdentity
        then some record
        else before.committedRecord identity)

def CommittedRecord
    {Identity : Type}
    (ledger : CommitmentLedger Identity)
    (runIdentity : Identity)
    (record : AdmissionRecord Identity) : Prop :=
  ledger.committedRecord runIdentity = some record

theorem atomic_record_commit_persists_acknowledgement
    {Identity : Type}
    [DecidableEq Identity]
    {before after : CommitmentLedger Identity}
    {runIdentity : Identity}
    {record : AdmissionRecord Identity}
    (commit : AtomicRecordCommit before after runIdentity record) :
    CommittedRecord after runIdentity record := by
  simpa [CommittedRecord] using commit.2.2 runIdentity

theorem first_linearized_commit_rejects_second_same_run
    {Identity : Type}
    [DecidableEq Identity]
    {before middle : CommitmentLedger Identity}
    {runIdentity : Identity}
    {firstRecord secondRecord : AdmissionRecord Identity}
    (firstCommit :
      AtomicRecordCommit before middle runIdentity firstRecord) :
    ¬CanRecordCommit middle runIdentity secondRecord := by
  intro secondCanCommit
  have persisted :=
    atomic_record_commit_persists_acknowledgement firstCommit
  have missing := secondCanCommit.2.1
  rw [persisted] at missing
  contradiction

theorem lost_acknowledgement_is_recoverable_without_recommit
    {Identity : Type}
    [DecidableEq Identity]
    {before after : CommitmentLedger Identity}
    {runIdentity : Identity}
    {record : AdmissionRecord Identity}
    (commit : AtomicRecordCommit before after runIdentity record) :
    CommittedRecord after runIdentity record
      ∧ ¬CanRecordCommit after runIdentity record := by
  exact ⟨
    atomic_record_commit_persists_acknowledgement commit,
    first_linearized_commit_rejects_second_same_run commit
  ⟩

def ConflictingRetry
    {Identity : Type}
    (ledger : CommitmentLedger Identity)
    (runIdentity : Identity)
    (requestedRecord : AdmissionRecord Identity) : Prop :=
  ∃ committedRecord,
    CommittedRecord ledger runIdentity committedRecord
      ∧ committedRecord ≠ requestedRecord

theorem conflicting_retry_returns_existing_record
    {Identity : Type}
    [DecidableEq Identity]
    {before after : CommitmentLedger Identity}
    {runIdentity : Identity}
    {committed requested : AdmissionRecord Identity}
    (commit : AtomicRecordCommit before after runIdentity committed)
    (different : committed ≠ requested) :
    ConflictingRetry after runIdentity requested := by
  exact ⟨
    committed,
    atomic_record_commit_persists_acknowledgement commit,
    different
  ⟩

def MarkerEquivalent
    {Identity : Type}
    (left right : CommitmentLedger Identity) : Prop :=
  ∀ runIdentity,
    (∃ record, CommittedRecord left runIdentity record)
      ↔ ∃ record, CommittedRecord right runIdentity record

def exampleRecordA : AdmissionRecord LedgerIdentity :=
  ⟨.baselineReceipt, .extendedReceipt⟩

def exampleRecordB : AdmissionRecord LedgerIdentity :=
  ⟨.extendedReceipt, .baselineReceipt⟩

def recordLedgerA : CommitmentLedger LedgerIdentity where
  consumedReceipt := fun _ => False
  committedRecord := fun identity =>
    if identity = .run then some exampleRecordA else none

def recordLedgerB : CommitmentLedger LedgerIdentity where
  consumedReceipt := fun _ => False
  committedRecord := fun identity =>
    if identity = .run then some exampleRecordB else none

theorem boolean_commit_marker_loses_acknowledgement_payload :
    MarkerEquivalent recordLedgerA recordLedgerB
      ∧ recordLedgerA.committedRecord .run ≠
        recordLedgerB.committedRecord .run := by
  constructor
  · intro identity
    cases identity <;>
      simp [CommittedRecord, recordLedgerA, recordLedgerB]
  · simp [recordLedgerA, recordLedgerB, exampleRecordA, exampleRecordB]

end SearchRouteAdmissionHistory
