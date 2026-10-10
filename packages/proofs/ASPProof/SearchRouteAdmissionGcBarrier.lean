-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteAdmissionRetention

namespace SearchRouteAdmissionGcBarrier

open SearchRouteAdmissionLedger
open SearchRouteAdmissionHistory
open SearchRouteAdmissionRetention

structure GcCycle (Identity : Type) where
  markEpoch : Nat
  sweepEpoch : Nat
  snapshotReferenced : Identity → Prop
  remembered : Identity → Prop

def WellFormedCycle
    {Identity : Type}
    (cycle : GcCycle Identity) : Prop :=
  cycle.markEpoch ≤ cycle.sweepEpoch

def ConcurrentDuringSweep
    {Identity : Type}
    (cycle : GcCycle Identity)
    (commitEpoch : Nat) : Prop :=
  cycle.markEpoch < commitEpoch ∧ commitEpoch ≤ cycle.sweepEpoch

def Protected
    {Identity : Type}
    (cycle : GcCycle Identity)
    (receiptIdentity : Identity) : Prop :=
  cycle.snapshotReferenced receiptIdentity
    ∨ cycle.remembered receiptIdentity

def SafeSweep
    {Identity Payload : Type}
    (cycle : GcCycle Identity)
    (before after : ReceiptStore Identity Payload) : Prop :=
  ∀ receiptIdentity,
    Protected cycle receiptIdentity →
    after receiptIdentity = before receiptIdentity

def BarrierCovers
    {Identity : Type}
    (cycle : GcCycle Identity)
    (record : AdmissionRecord Identity) : Prop :=
  cycle.remembered record.baselineReceiptIdentity
    ∧ cycle.remembered record.extendedReceiptIdentity

structure ConcurrentCommitCertificate
    {Identity : Type}
    (cycle : GcCycle Identity)
    (commitEpoch : Nat)
    (record : AdmissionRecord Identity) : Prop where
  duringSweep : ConcurrentDuringSweep cycle commitEpoch
  barrierInstalledBeforePublication : BarrierCovers cycle record

theorem barrier_safe_sweep_preserves_concurrent_commit_receipts
    {Identity Payload : Type}
    {cycle : GcCycle Identity}
    {commitEpoch : Nat}
    {record : AdmissionRecord Identity}
    {before after : ReceiptStore Identity Payload}
    (commit :
      ConcurrentCommitCertificate cycle commitEpoch record)
    (safeSweep : SafeSweep cycle before after) :
    after record.baselineReceiptIdentity =
        before record.baselineReceiptIdentity
      ∧ after record.extendedReceiptIdentity =
        before record.extendedReceiptIdentity := by
  exact ⟨
    safeSweep record.baselineReceiptIdentity
      (Or.inr commit.barrierInstalledBeforePublication.1),
    safeSweep record.extendedReceiptIdentity
      (Or.inr commit.barrierInstalledBeforePublication.2)
  ⟩

def SnapshotOnlySafeSweep
    {Identity Payload : Type}
    (cycle : GcCycle Identity)
    (before after : ReceiptStore Identity Payload) : Prop :=
  ∀ receiptIdentity,
    cycle.snapshotReferenced receiptIdentity →
    after receiptIdentity = before receiptIdentity

def emptySnapshotCycle : GcCycle LedgerIdentity where
  markEpoch := 0
  sweepEpoch := 2
  snapshotReferenced := fun _ => False
  remembered := fun _ => False

def sweptReceiptStore : ReceiptStore LedgerIdentity Bool :=
  fun _ => none

theorem snapshot_only_gc_can_delete_concurrently_referenced_receipts :
    WellFormedCycle emptySnapshotCycle
      ∧ ConcurrentDuringSweep emptySnapshotCycle 1
      ∧ SnapshotOnlySafeSweep emptySnapshotCycle completeReceiptStore
        sweptReceiptStore
      ∧ completeReceiptStore exampleRecordA.baselineReceiptIdentity =
        some false
      ∧ sweptReceiptStore exampleRecordA.baselineReceiptIdentity = none
      ∧ ¬BarrierCovers emptySnapshotCycle exampleRecordA := by
  simp [WellFormedCycle, ConcurrentDuringSweep, SnapshotOnlySafeSweep,
    emptySnapshotCycle, completeReceiptStore, sweptReceiptStore,
    exampleRecordA, BarrierCovers]

theorem publication_during_sweep_requires_both_barrier_entries
    {Identity : Type}
    {cycle : GcCycle Identity}
    {commitEpoch : Nat}
    {record : AdmissionRecord Identity}
    (certificate :
      ConcurrentCommitCertificate cycle commitEpoch record) :
    cycle.remembered record.baselineReceiptIdentity
      ∧ cycle.remembered record.extendedReceiptIdentity :=
  certificate.barrierInstalledBeforePublication

def RememberedSetMonotone
    {Identity : Type}
    (before after : GcCycle Identity) : Prop :=
  ∀ receiptIdentity,
    before.remembered receiptIdentity →
    after.remembered receiptIdentity

theorem remembered_set_monotonicity_preserves_installed_barrier
    {Identity : Type}
    {before after : GcCycle Identity}
    {record : AdmissionRecord Identity}
    (monotone : RememberedSetMonotone before after)
    (covered : BarrierCovers before record) :
    BarrierCovers after record := by
  exact ⟨
    monotone record.baselineReceiptIdentity covered.1,
    monotone record.extendedReceiptIdentity covered.2
  ⟩

end SearchRouteAdmissionGcBarrier
