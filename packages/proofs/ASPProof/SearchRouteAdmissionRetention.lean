-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteAdmissionHistory

namespace SearchRouteAdmissionRetention

open SearchRouteAdmissionLedger
open SearchRouteAdmissionHistory

abbrev ReceiptStore (Identity Payload : Type) :=
  Identity → Option Payload

structure RetentionPolicy (Identity : Type) where
  retainUntil : Identity → Nat

def RetainedAt
    {Identity : Type}
    (policy : RetentionPolicy Identity)
    (now : Nat)
    (runIdentity : Identity) : Prop :=
  now ≤ policy.retainUntil runIdentity

def AuthorizedRetirement
    {Identity : Type}
    (policy : RetentionPolicy Identity)
    (now : Nat)
    (runIdentity : Identity) : Prop :=
  policy.retainUntil runIdentity < now

theorem run_cannot_retire_before_retention_deadline
    {Identity : Type}
    {policy : RetentionPolicy Identity}
    {now : Nat}
    {runIdentity : Identity}
    (retained : RetainedAt policy now runIdentity) :
    ¬AuthorizedRetirement policy now runIdentity :=
  Nat.not_lt_of_ge retained

def RecordResolvable
    {Identity Payload : Type}
    (commitPayload : Payload → Identity)
    (store : ReceiptStore Identity Payload)
    (record : AdmissionRecord Identity) : Prop :=
  ∃ baselinePayload extensionPayload,
    store record.baselineReceiptIdentity = some baselinePayload
      ∧ store record.extendedReceiptIdentity = some extensionPayload
      ∧ commitPayload baselinePayload = record.baselineReceiptIdentity
      ∧ commitPayload extensionPayload = record.extendedReceiptIdentity

def AcknowledgementRecoverable
    {Identity Payload : Type}
    (commitPayload : Payload → Identity)
    (ledger : CommitmentLedger Identity)
    (store : ReceiptStore Identity Payload)
    (runIdentity : Identity) : Prop :=
  ∃ record,
    CommittedRecord ledger runIdentity record
      ∧ RecordResolvable commitPayload store record

def RetentionInvariant
    {Identity Payload : Type}
    (retainedRun : Identity → Prop)
    (commitPayload : Payload → Identity)
    (ledger : CommitmentLedger Identity)
    (store : ReceiptStore Identity Payload) : Prop :=
  ∀ runIdentity,
    retainedRun runIdentity →
    ∀ record,
      CommittedRecord ledger runIdentity record →
      RecordResolvable commitPayload store record

theorem retained_committed_run_has_recoverable_acknowledgement
    {Identity Payload : Type}
    {retainedRun : Identity → Prop}
    {commitPayload : Payload → Identity}
    {ledger : CommitmentLedger Identity}
    {store : ReceiptStore Identity Payload}
    {runIdentity : Identity}
    {record : AdmissionRecord Identity}
    (retention :
      RetentionInvariant retainedRun commitPayload ledger store)
    (retained : retainedRun runIdentity)
    (committed : CommittedRecord ledger runIdentity record) :
    AcknowledgementRecoverable commitPayload ledger store runIdentity := by
  exact ⟨record, committed, retention runIdentity retained record committed⟩

def ReferencedReceipt
    {Identity : Type}
    (retainedRun : Identity → Prop)
    (ledger : CommitmentLedger Identity)
    (receiptIdentity : Identity) : Prop :=
  ∃ runIdentity record,
    retainedRun runIdentity
      ∧ CommittedRecord ledger runIdentity record
      ∧ (receiptIdentity = record.baselineReceiptIdentity
        ∨ receiptIdentity = record.extendedReceiptIdentity)

def SafeGarbageCollection
    {Identity Payload : Type}
    (retainedRun : Identity → Prop)
    (ledger : CommitmentLedger Identity)
    (before after : ReceiptStore Identity Payload) : Prop :=
  ∀ receiptIdentity,
    ReferencedReceipt retainedRun ledger receiptIdentity →
    after receiptIdentity = before receiptIdentity

theorem safe_gc_preserves_record_resolvability
    {Identity Payload : Type}
    {retainedRun : Identity → Prop}
    {commitPayload : Payload → Identity}
    {ledger : CommitmentLedger Identity}
    {before after : ReceiptStore Identity Payload}
    {runIdentity : Identity}
    {record : AdmissionRecord Identity}
    (safeGc :
      SafeGarbageCollection retainedRun ledger before after)
    (retained : retainedRun runIdentity)
    (committed : CommittedRecord ledger runIdentity record)
    (resolvable : RecordResolvable commitPayload before record) :
    RecordResolvable commitPayload after record := by
  obtain ⟨baselinePayload, extensionPayload, baselineStored,
    extensionStored, baselineCommitted, extensionCommitted⟩ := resolvable
  refine ⟨
    baselinePayload,
    extensionPayload,
    ?_,
    ?_,
    baselineCommitted,
    extensionCommitted
  ⟩
  · calc
      after record.baselineReceiptIdentity =
          before record.baselineReceiptIdentity :=
        safeGc record.baselineReceiptIdentity
          ⟨runIdentity, record, retained, committed, Or.inl rfl⟩
      _ = some baselinePayload := baselineStored
  · calc
      after record.extendedReceiptIdentity =
          before record.extendedReceiptIdentity :=
        safeGc record.extendedReceiptIdentity
          ⟨runIdentity, record, retained, committed, Or.inr rfl⟩
      _ = some extensionPayload := extensionStored

theorem safe_gc_preserves_retention_invariant
    {Identity Payload : Type}
    {retainedRun : Identity → Prop}
    {commitPayload : Payload → Identity}
    {ledger : CommitmentLedger Identity}
    {before after : ReceiptStore Identity Payload}
    (retention :
      RetentionInvariant retainedRun commitPayload ledger before)
    (safeGc :
      SafeGarbageCollection retainedRun ledger before after) :
    RetentionInvariant retainedRun commitPayload ledger after := by
  intro runIdentity retained record committed
  exact safe_gc_preserves_record_resolvability safeGc retained committed
    (retention runIdentity retained record committed)

def emptyReceiptStore : ReceiptStore LedgerIdentity Unit :=
  fun _ => none

def unitPayloadCommit : Unit → LedgerIdentity :=
  fun _ => .baselineReceipt

theorem committed_record_does_not_imply_payload_reachability :
    CommittedRecord recordLedgerA .run exampleRecordA
      ∧ ¬AcknowledgementRecoverable unitPayloadCommit recordLedgerA
        emptyReceiptStore .run := by
  constructor
  · simp [CommittedRecord, recordLedgerA, exampleRecordA]
  · intro recoverable
    obtain ⟨record, _, baselinePayload, _, baselineStored, _⟩ :=
      recoverable
    simp [emptyReceiptStore] at baselineStored

def examplePayloadCommit : Bool → LedgerIdentity
  | false => .baselineReceipt
  | true => .extendedReceipt

def completeReceiptStore : ReceiptStore LedgerIdentity Bool
  | .run => none
  | .baselineReceipt => some false
  | .extendedReceipt => some true

theorem deleting_referenced_payload_breaks_acknowledgement_recovery :
    RecordResolvable examplePayloadCommit completeReceiptStore exampleRecordA
      ∧ ¬RecordResolvable examplePayloadCommit
        (fun _ => none) exampleRecordA := by
  constructor
  · exact ⟨false, true, by rfl, by rfl, by rfl, by rfl⟩
  · intro resolvable
    obtain ⟨_, _, baselineStored, _⟩ := resolvable
    simp at baselineStored

end SearchRouteAdmissionRetention
