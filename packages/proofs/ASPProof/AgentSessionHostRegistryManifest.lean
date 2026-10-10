-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.AgentSessionHostRegistryCompaction

namespace ASPProof.AgentSessionHostRegistryManifest

open ASPProof.AgentSessionHostRegistryCompaction

inductive ManifestDisposition where
  | live
  | tombstoned
  deriving DecidableEq, Repr

structure ManifestRecord where
  key : KeyId
  generation : EntryGeneration
  disposition : ManifestDisposition
  deriving DecidableEq, Repr

structure ManifestSnapshot where
  snapshotRevision : MultiKeySnapshotRevision
  records : List ManifestRecord
  entries : List RegistryEntry
  gcFloor : GcFloor
  deriving DecidableEq, Repr

inductive StrictlyOrderedKeys : List ManifestRecord → Prop where
  | nil : StrictlyOrderedKeys []
  | singleton (record : ManifestRecord) :
      StrictlyOrderedKeys [record]
  | cons
      (first second : ManifestRecord)
      (rest : List ManifestRecord)
      (less : first.key < second.key)
      (orderedTail : StrictlyOrderedKeys (second :: rest)) :
      StrictlyOrderedKeys (first :: second :: rest)

inductive ManifestAccounts (floor : GcFloor) :
    List ManifestRecord → List RegistryEntry → Prop where
  | nil : ManifestAccounts floor [] []
  | liveEmpty
      (key : KeyId)
      (generation : EntryGeneration)
      (records : List ManifestRecord)
      (entries : List RegistryEntry)
      (rest : ManifestAccounts floor records entries) :
      ManifestAccounts floor
        ({ key := key, generation := generation,
           disposition := .live } :: records)
        ({ key := key, generation := generation,
           state := .empty } :: entries)
  | liveAccepted
      (key : KeyId)
      (generation : EntryGeneration)
      (receipt : ReceiptId)
      (records : List ManifestRecord)
      (entries : List RegistryEntry)
      (rest : ManifestAccounts floor records entries) :
      ManifestAccounts floor
        ({ key := key, generation := generation,
           disposition := .live } :: records)
        ({ key := key, generation := generation,
           state := .accepted receipt } :: entries)
  | tombstonePresent
      (key : KeyId)
      (generation : EntryGeneration)
      (records : List ManifestRecord)
      (entries : List RegistryEntry)
      (covered : generation ≤ floor)
      (rest : ManifestAccounts floor records entries) :
      ManifestAccounts floor
        ({ key := key, generation := generation,
           disposition := .tombstoned } :: records)
        ({ key := key, generation := generation,
           state := .retired } :: entries)
  | tombstoneCompacted
      (key : KeyId)
      (generation : EntryGeneration)
      (records : List ManifestRecord)
      (entries : List RegistryEntry)
      (covered : generation ≤ floor)
      (rest : ManifestAccounts floor records entries) :
      ManifestAccounts floor
        ({ key := key, generation := generation,
           disposition := .tombstoned } :: records)
        entries

inductive CanonicalManifest : ManifestSnapshot → Prop where
  | certified
      (snapshotRevision : MultiKeySnapshotRevision)
      (records : List ManifestRecord)
      (entries : List RegistryEntry)
      (gcFloor : GcFloor)
      (ordered : StrictlyOrderedKeys records)
      (accounts : ManifestAccounts gcFloor records entries) :
      CanonicalManifest {
        snapshotRevision := snapshotRevision
        records := records
        entries := entries
        gcFloor := gcFloor
      }

inductive DuplicateKeyManifest : List ManifestRecord → Prop where
  | pair
      (key : KeyId)
      (leftGeneration rightGeneration : EntryGeneration)
      (leftDisposition rightDisposition : ManifestDisposition) :
      DuplicateKeyManifest [
        { key := key, generation := leftGeneration,
          disposition := leftDisposition },
        { key := key, generation := rightGeneration,
          disposition := rightDisposition }
      ]

inductive OrphanEntrySnapshot : ManifestSnapshot → Prop where
  | singleton
      (snapshotRevision : MultiKeySnapshotRevision)
      (entry : RegistryEntry)
      (gcFloor : GcFloor) :
      OrphanEntrySnapshot {
        snapshotRevision := snapshotRevision
        records := []
        entries := [entry]
        gcFloor := gcFloor
      }

inductive UnaccountedLiveKeySnapshot : ManifestSnapshot → Prop where
  | singleton
      (snapshotRevision : MultiKeySnapshotRevision)
      (key : KeyId)
      (generation : EntryGeneration)
      (gcFloor : GcFloor) :
      UnaccountedLiveKeySnapshot {
        snapshotRevision := snapshotRevision
        records := [{
          key := key
          generation := generation
          disposition := .live
        }]
        entries := []
        gcFloor := gcFloor
      }

structure CanonicalManifestInput where
  schemaVersion : Nat
  serializationVersion : Nat
  snapshotRevision : MultiKeySnapshotRevision
  records : List ManifestRecord
  entries : List RegistryEntry
  gcFloor : GcFloor
  deriving DecidableEq, Repr

def canonicalManifestInput (snapshot : ManifestSnapshot) :
    CanonicalManifestInput :=
  {
    schemaVersion := 1
    serializationVersion := 1
    snapshotRevision := snapshot.snapshotRevision
    records := snapshot.records
    entries := snapshot.entries
    gcFloor := snapshot.gcFloor
  }

inductive KeyScopedTombstoneCompaction :
    ManifestSnapshot → ManifestSnapshot → Prop where
  | compact
      (snapshotRevision : MultiKeySnapshotRevision)
      (key : KeyId)
      (generation : EntryGeneration)
      (gcFloor : GcFloor)
      (covered : generation ≤ gcFloor) :
      KeyScopedTombstoneCompaction
        {
          snapshotRevision := snapshotRevision
          records := [{
            key := key
            generation := generation
            disposition := .tombstoned
          }]
          entries := [{
            key := key
            generation := generation
            state := .retired
          }]
          gcFloor := gcFloor
        }
        {
          snapshotRevision := snapshotRevision + 1
          records := [{
            key := key
            generation := generation
            disposition := .tombstoned
          }]
          entries := []
          gcFloor := gcFloor
        }

structure ManifestLedgerHead where
  revision : MultiKeyLedgerRevision
  snapshot : ManifestSnapshot
  deriving DecidableEq, Repr

abbrev CanonicalManifestChoice :=
  MultiKeyLedgerRevision → ManifestSnapshot

inductive CanonicalManifestCas
    (choice : CanonicalManifestChoice) :
    MultiKeyLedgerRevision →
    ManifestLedgerHead → ManifestLedgerHead → Prop where
  | commit
      (revision : MultiKeyLedgerRevision)
      (before : ManifestSnapshot)
      (canonical : CanonicalManifest (choice (revision + 1))) :
      CanonicalManifestCas choice revision
        { revision := revision, snapshot := before }
        { revision := revision + 1, snapshot := choice (revision + 1) }

theorem duplicate_manifest_constructible
    (key : KeyId)
    (leftGeneration rightGeneration : EntryGeneration)
    (leftDisposition rightDisposition : ManifestDisposition) :
    DuplicateKeyManifest [
      { key := key, generation := leftGeneration,
        disposition := leftDisposition },
      { key := key, generation := rightGeneration,
        disposition := rightDisposition }
    ] := by
  exact DuplicateKeyManifest.pair
    key leftGeneration rightGeneration
    leftDisposition rightDisposition

theorem orphan_entry_constructible
    (snapshotRevision : MultiKeySnapshotRevision)
    (entry : RegistryEntry)
    (gcFloor : GcFloor) :
    OrphanEntrySnapshot {
      snapshotRevision := snapshotRevision
      records := []
      entries := [entry]
      gcFloor := gcFloor
    } := by
  exact OrphanEntrySnapshot.singleton snapshotRevision entry gcFloor

theorem unaccounted_live_key_constructible
    (snapshotRevision : MultiKeySnapshotRevision)
    (key : KeyId)
    (generation : EntryGeneration)
    (gcFloor : GcFloor) :
    UnaccountedLiveKeySnapshot {
      snapshotRevision := snapshotRevision
      records := [{
        key := key
        generation := generation
        disposition := .live
      }]
      entries := []
      gcFloor := gcFloor
    } := by
  exact UnaccountedLiveKeySnapshot.singleton
    snapshotRevision key generation gcFloor

theorem duplicate_pair_not_strictly_ordered
    (key : KeyId)
    (leftGeneration rightGeneration : EntryGeneration)
    (leftDisposition rightDisposition : ManifestDisposition) :
    ¬ StrictlyOrderedKeys [
      { key := key, generation := leftGeneration,
        disposition := leftDisposition },
      { key := key, generation := rightGeneration,
        disposition := rightDisposition }
    ] := by
  intro ordered
  cases ordered with
  | cons first second rest less orderedTail =>
      exact Nat.lt_irrefl key less

theorem reverse_pair_not_strictly_ordered
    (leftKey rightKey : KeyId)
    (leftGeneration rightGeneration : EntryGeneration)
    (leftDisposition rightDisposition : ManifestDisposition)
    (less : leftKey < rightKey) :
    ¬ StrictlyOrderedKeys [
      { key := rightKey, generation := rightGeneration,
        disposition := rightDisposition },
      { key := leftKey, generation := leftGeneration,
        disposition := leftDisposition }
    ] := by
  intro ordered
  cases ordered with
  | cons first second rest reverseLess orderedTail =>
      exact (Nat.not_lt_of_ge (Nat.le_of_lt less)) reverseLess

theorem live_accepted_record_is_accounted
    (floor : GcFloor)
    (key : KeyId)
    (generation : EntryGeneration)
    (receipt : ReceiptId) :
    ManifestAccounts floor
      [{ key := key, generation := generation,
         disposition := .live }]
      [{ key := key, generation := generation,
         state := .accepted receipt }] := by
  exact ManifestAccounts.liveAccepted
    key generation receipt [] [] ManifestAccounts.nil

theorem present_tombstone_record_is_accounted
    (floor : GcFloor)
    (key : KeyId)
    (generation : EntryGeneration)
    (covered : generation ≤ floor) :
    ManifestAccounts floor
      [{ key := key, generation := generation,
         disposition := .tombstoned }]
      [{ key := key, generation := generation,
         state := .retired }] := by
  exact ManifestAccounts.tombstonePresent
    key generation [] [] covered ManifestAccounts.nil

theorem compacted_tombstone_record_is_accounted
    (floor : GcFloor)
    (key : KeyId)
    (generation : EntryGeneration)
    (covered : generation ≤ floor) :
    ManifestAccounts floor
      [{ key := key, generation := generation,
         disposition := .tombstoned }]
      [] := by
  exact ManifestAccounts.tombstoneCompacted
    key generation [] [] covered ManifestAccounts.nil

theorem orphan_entry_not_accounted
    (floor : GcFloor)
    (entry : RegistryEntry) :
    ¬ ManifestAccounts floor [] [entry] := by
  intro accounts
  cases accounts

theorem live_key_without_entry_not_accounted
    (floor : GcFloor)
    (key : KeyId)
    (generation : EntryGeneration) :
    ¬ ManifestAccounts floor
      [{ key := key, generation := generation,
         disposition := .live }]
      [] := by
  intro accounts
  cases accounts

theorem canonical_live_snapshot_constructible
    (snapshotRevision : MultiKeySnapshotRevision)
    (floor : GcFloor)
    (key : KeyId)
    (generation : EntryGeneration)
    (receipt : ReceiptId) :
    CanonicalManifest {
      snapshotRevision := snapshotRevision
      records := [{ key := key, generation := generation,
                    disposition := .live }]
      entries := [{ key := key, generation := generation,
                    state := .accepted receipt }]
      gcFloor := floor
    } := by
  exact CanonicalManifest.certified
    snapshotRevision _ _ floor
    (StrictlyOrderedKeys.singleton _)
    (live_accepted_record_is_accounted floor key generation receipt)

theorem canonical_manifest_has_ordering
    {snapshot : ManifestSnapshot}
    (canonical : CanonicalManifest snapshot) :
    StrictlyOrderedKeys snapshot.records := by
  cases canonical
  assumption

theorem canonical_manifest_has_accounting
    {snapshot : ManifestSnapshot}
    (canonical : CanonicalManifest snapshot) :
    ManifestAccounts snapshot.gcFloor snapshot.records snapshot.entries := by
  cases canonical
  assumption

theorem duplicate_snapshot_not_canonical
    (snapshotRevision : MultiKeySnapshotRevision)
    (floor : GcFloor)
    (key : KeyId)
    (leftGeneration rightGeneration : EntryGeneration)
    (leftReceipt rightReceipt : ReceiptId) :
    ¬ CanonicalManifest {
      snapshotRevision := snapshotRevision
      records := [
        { key := key, generation := leftGeneration,
          disposition := .live },
        { key := key, generation := rightGeneration,
          disposition := .live }
      ]
      entries := [
        { key := key, generation := leftGeneration,
          state := .accepted leftReceipt },
        { key := key, generation := rightGeneration,
          state := .accepted rightReceipt }
      ]
      gcFloor := floor
    } := by
  intro canonical
  exact duplicate_pair_not_strictly_ordered
    key leftGeneration rightGeneration .live .live
    (canonical_manifest_has_ordering canonical)

theorem orphan_snapshot_not_canonical
    (snapshotRevision : MultiKeySnapshotRevision)
    (floor : GcFloor)
    (entry : RegistryEntry) :
    ¬ CanonicalManifest {
      snapshotRevision := snapshotRevision
      records := []
      entries := [entry]
      gcFloor := floor
    } := by
  intro canonical
  exact orphan_entry_not_accounted floor entry
    (canonical_manifest_has_accounting canonical)

theorem unaccounted_live_snapshot_not_canonical
    (snapshotRevision : MultiKeySnapshotRevision)
    (floor : GcFloor)
    (key : KeyId)
    (generation : EntryGeneration) :
    ¬ CanonicalManifest {
      snapshotRevision := snapshotRevision
      records := [{ key := key, generation := generation,
                    disposition := .live }]
      entries := []
      gcFloor := floor
    } := by
  intro canonical
  exact live_key_without_entry_not_accounted
    floor key generation
    (canonical_manifest_has_accounting canonical)

theorem canonical_input_binds_records
    (snapshot : ManifestSnapshot) :
    (canonicalManifestInput snapshot).records = snapshot.records := by
  rfl

theorem canonical_input_binds_entries
    (snapshot : ManifestSnapshot) :
    (canonicalManifestInput snapshot).entries = snapshot.entries := by
  rfl

theorem canonical_input_binds_revision_and_floor
    (snapshot : ManifestSnapshot) :
    (canonicalManifestInput snapshot).snapshotRevision =
      snapshot.snapshotRevision ∧
    (canonicalManifestInput snapshot).gcFloor = snapshot.gcFloor := by
  exact ⟨rfl, rfl⟩

theorem tombstone_compaction_preserves_record
    {before after : ManifestSnapshot}
    (compaction : KeyScopedTombstoneCompaction before after) :
    after.records = before.records := by
  cases compaction
  rfl

theorem compacted_tombstone_snapshot_is_canonical
    (snapshotRevision : MultiKeySnapshotRevision)
    (floor : GcFloor)
    (key : KeyId)
    (generation : EntryGeneration)
    (covered : generation ≤ floor) :
    CanonicalManifest {
      snapshotRevision := snapshotRevision
      records := [{ key := key, generation := generation,
                    disposition := .tombstoned }]
      entries := []
      gcFloor := floor
    } := by
  exact CanonicalManifest.certified
    snapshotRevision _ _ floor
    (StrictlyOrderedKeys.singleton _)
    (compacted_tombstone_record_is_accounted
      floor key generation covered)

theorem stale_manifest_revision_cannot_commit
    (choice : CanonicalManifestChoice)
    (expected current : MultiKeyLedgerRevision)
    (snapshot : ManifestSnapshot)
    (stale : expected ≠ current) :
    ¬ ∃ after,
      CanonicalManifestCas choice expected
        { revision := current, snapshot := snapshot }
        after := by
  intro witness
  rcases witness with ⟨after, commit⟩
  cases commit
  exact stale rfl

theorem canonical_same_head_manifest_commits_are_unique
    (choice : CanonicalManifestChoice)
    (expected : MultiKeyLedgerRevision)
    (before left right : ManifestLedgerHead)
    (leftCommit : CanonicalManifestCas choice expected before left)
    (rightCommit : CanonicalManifestCas choice expected before right) :
    left = right := by
  cases leftCommit
  cases rightCommit
  rfl

end ASPProof.AgentSessionHostRegistryManifest
