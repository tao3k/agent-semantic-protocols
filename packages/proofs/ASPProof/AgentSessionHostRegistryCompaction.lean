import ASPProof.AgentSessionHostRegistryLedger

namespace ASPProof.AgentSessionHostRegistryCompaction

open ASPProof.AgentSessionHostAcceptanceDurability

abbrev KeyId := Nat
abbrev ReceiptId := Nat
abbrev EntryGeneration := Nat
abbrev MultiKeySnapshotRevision := Nat
abbrev MultiKeyLedgerRevision := Nat
abbrev GcFloor := Nat

inductive EntryState where
  | empty
  | accepted (receipt : ReceiptId)
  | retired
  deriving DecidableEq, Repr

structure RegistryEntry where
  key : KeyId
  generation : EntryGeneration
  state : EntryState
  deriving DecidableEq, Repr

structure MultiKeySnapshot where
  snapshotRevision : MultiKeySnapshotRevision
  manifest : List KeyId
  entries : List RegistryEntry
  gcFloor : GcFloor
  deriving DecidableEq, Repr

structure MultiKeyLedgerHead where
  revision : MultiKeyLedgerRevision
  snapshot : MultiKeySnapshot
  deriving DecidableEq, Repr

inductive CompactsEntries (floor : GcFloor) :
    List RegistryEntry → List RegistryEntry → Prop where
  | nil : CompactsEntries floor [] []
  | keep
      (entry : RegistryEntry)
      (before after : List RegistryEntry)
      (rest : CompactsEntries floor before after) :
      CompactsEntries floor (entry :: before) (entry :: after)
  | dropRetired
      (key : KeyId)
      (generation : EntryGeneration)
      (before after : List RegistryEntry)
      (covered : generation ≤ floor)
      (rest : CompactsEntries floor before after) :
      CompactsEntries floor
        ({ key := key, generation := generation, state := .retired } :: before)
        after

inductive FloorSafeForEntries (floor : GcFloor) :
    List RegistryEntry → Prop where
  | nil : FloorSafeForEntries floor []
  | empty
      (key : KeyId)
      (generation : EntryGeneration)
      (rest : List RegistryEntry)
      (aboveFloor : floor < generation)
      (safeRest : FloorSafeForEntries floor rest) :
      FloorSafeForEntries floor
        ({ key := key, generation := generation, state := .empty } :: rest)
  | accepted
      (key : KeyId)
      (generation : EntryGeneration)
      (receipt : ReceiptId)
      (rest : List RegistryEntry)
      (aboveFloor : floor < generation)
      (safeRest : FloorSafeForEntries floor rest) :
      FloorSafeForEntries floor
        ({ key := key, generation := generation,
           state := .accepted receipt } :: rest)
  | retired
      (key : KeyId)
      (generation : EntryGeneration)
      (rest : List RegistryEntry)
      (safeRest : FloorSafeForEntries floor rest) :
      FloorSafeForEntries floor
        ({ key := key, generation := generation, state := .retired } :: rest)

inductive SafeMultiKeyCompaction :
    MultiKeySnapshot → MultiKeySnapshot → Prop where
  | compact
      (snapshotRevision : MultiKeySnapshotRevision)
      (manifest : List KeyId)
      (beforeEntries afterEntries : List RegistryEntry)
      (oldFloor newFloor : GcFloor)
      (floorMonotone : oldFloor ≤ newFloor)
      (entries : CompactsEntries newFloor beforeEntries afterEntries)
      (floorSafe : FloorSafeForEntries newFloor afterEntries) :
      SafeMultiKeyCompaction
        {
          snapshotRevision := snapshotRevision
          manifest := manifest
          entries := beforeEntries
          gcFloor := oldFloor
        }
        {
          snapshotRevision := snapshotRevision + 1
          manifest := manifest
          entries := afterEntries
          gcFloor := newFloor
        }

inductive NaiveCompaction : MultiKeySnapshot → MultiKeySnapshot → Prop where
  | project
      (before after : MultiKeySnapshot) :
      NaiveCompaction before after

inductive PartialAcceptedLoss :
    MultiKeySnapshot → MultiKeySnapshot → KeyId → Prop where
  | dropRight
      (snapshotRevision : MultiKeySnapshotRevision)
      (leftKey rightKey : KeyId)
      (leftGeneration rightGeneration : EntryGeneration)
      (leftReceipt rightReceipt : ReceiptId)
      (gcFloor : GcFloor) :
      PartialAcceptedLoss
        {
          snapshotRevision := snapshotRevision
          manifest := [leftKey, rightKey]
          entries := [
            { key := leftKey, generation := leftGeneration,
              state := .accepted leftReceipt },
            { key := rightKey, generation := rightGeneration,
              state := .accepted rightReceipt }
          ]
          gcFloor := gcFloor
        }
        {
          snapshotRevision := snapshotRevision + 1
          manifest := [leftKey, rightKey]
          entries := [
            { key := leftKey, generation := leftGeneration,
              state := .accepted leftReceipt }
          ]
          gcFloor := gcFloor
        }
        rightKey

inductive ImportAdmission (floor : GcFloor) :
    RegistryEntry → RecoveryDisposition → Prop where
  | current
      (entry : RegistryEntry)
      (aboveFloor : floor < entry.generation) :
      ImportAdmission floor entry .resume
  | covered
      (entry : RegistryEntry)
      (atOrBelowFloor : entry.generation ≤ floor) :
      ImportAdmission floor entry .quarantine

abbrev CanonicalMultiKeyChoice :=
  MultiKeyLedgerRevision → MultiKeySnapshot

inductive CanonicalCompactionCas
    (choice : CanonicalMultiKeyChoice) :
    MultiKeyLedgerRevision →
    MultiKeyLedgerHead → MultiKeyLedgerHead → Prop where
  | commit
      (revision : MultiKeyLedgerRevision)
      (before : MultiKeySnapshot)
      (safe : SafeMultiKeyCompaction before (choice (revision + 1))) :
      CanonicalCompactionCas choice revision
        { revision := revision, snapshot := before }
        { revision := revision + 1, snapshot := choice (revision + 1) }

inductive ContainsAccepted :
    List RegistryEntry → KeyId → ReceiptId → Prop where
  | head
      (key : KeyId)
      (generation : EntryGeneration)
      (receipt : ReceiptId)
      (rest : List RegistryEntry) :
      ContainsAccepted
        ({ key := key, generation := generation,
           state := .accepted receipt } :: rest)
        key receipt
  | tail
      (entry : RegistryEntry)
      (rest : List RegistryEntry)
      (key : KeyId)
      (receipt : ReceiptId)
      (contains : ContainsAccepted rest key receipt) :
      ContainsAccepted (entry :: rest) key receipt

inductive MultiKeyDelivered :
    MultiKeyLedgerHead → KeyId → ReceiptId → Prop where
  | verified
      (head : MultiKeyLedgerHead)
      (key : KeyId)
      (receipt : ReceiptId)
      (contains : ContainsAccepted head.snapshot.entries key receipt) :
      MultiKeyDelivered head key receipt

theorem naive_partial_accepted_loss_constructible
    (snapshotRevision : MultiKeySnapshotRevision)
    (leftKey rightKey : KeyId)
    (leftGeneration rightGeneration : EntryGeneration)
    (leftReceipt rightReceipt : ReceiptId)
    (gcFloor : GcFloor) :
    ∃ before after,
      NaiveCompaction before after ∧
      PartialAcceptedLoss before after rightKey := by
  exact ⟨
    {
      snapshotRevision := snapshotRevision
      manifest := [leftKey, rightKey]
      entries := [
        { key := leftKey, generation := leftGeneration,
          state := .accepted leftReceipt },
        { key := rightKey, generation := rightGeneration,
          state := .accepted rightReceipt }
      ]
      gcFloor := gcFloor
    },
    {
      snapshotRevision := snapshotRevision + 1
      manifest := [leftKey, rightKey]
      entries := [
        { key := leftKey, generation := leftGeneration,
          state := .accepted leftReceipt }
      ]
      gcFloor := gcFloor
    },
    NaiveCompaction.project _ _,
    PartialAcceptedLoss.dropRight
      snapshotRevision leftKey rightKey
      leftGeneration rightGeneration leftReceipt rightReceipt gcFloor
  ⟩

theorem accepted_singleton_cannot_disappear
    (floor : GcFloor)
    (key : KeyId)
    (generation : EntryGeneration)
    (receipt : ReceiptId) :
    ¬ CompactsEntries floor
      [{ key := key, generation := generation, state := .accepted receipt }]
      [] := by
  intro compaction
  cases compaction

theorem two_accepted_entries_cannot_partially_compact
    (floor : GcFloor)
    (leftKey rightKey : KeyId)
    (leftGeneration rightGeneration : EntryGeneration)
    (leftReceipt rightReceipt : ReceiptId) :
    ¬ CompactsEntries floor
      [
        { key := leftKey, generation := leftGeneration,
          state := .accepted leftReceipt },
        { key := rightKey, generation := rightGeneration,
          state := .accepted rightReceipt }
      ]
      [
        { key := leftKey, generation := leftGeneration,
          state := .accepted leftReceipt }
      ] := by
  intro compaction
  cases compaction with
  | keep _ _ _ rest =>
      exact accepted_singleton_cannot_disappear
        floor rightKey rightGeneration rightReceipt rest

theorem covered_retired_entry_can_compact
    (floor : GcFloor)
    (key : KeyId)
    (generation : EntryGeneration)
    (covered : generation ≤ floor) :
    CompactsEntries floor
      [{ key := key, generation := generation, state := .retired }]
      [] := by
  exact CompactsEntries.dropRetired
    key generation [] [] covered CompactsEntries.nil

theorem safe_compaction_preserves_manifest
    {before after : MultiKeySnapshot}
    (compaction : SafeMultiKeyCompaction before after) :
    after.manifest = before.manifest := by
  cases compaction
  rfl

theorem safe_compaction_advances_snapshot_revision
    {before after : MultiKeySnapshot}
    (compaction : SafeMultiKeyCompaction before after) :
    after.snapshotRevision = before.snapshotRevision + 1 := by
  cases compaction
  rfl

theorem safe_compaction_preserves_gc_floor_monotonicity
    {before after : MultiKeySnapshot}
    (compaction : SafeMultiKeyCompaction before after) :
    before.gcFloor ≤ after.gcFloor := by
  cases compaction
  assumption

theorem accepted_at_or_below_floor_not_safe
    (floor : GcFloor)
    (key : KeyId)
    (generation : EntryGeneration)
    (receipt : ReceiptId)
    (covered : generation ≤ floor) :
    ¬ FloorSafeForEntries floor
      [{ key := key, generation := generation,
         state := .accepted receipt }] := by
  intro safe
  cases safe with
  | accepted _ _ _ _ aboveFloor _ =>
      exact (Nat.not_lt_of_ge covered) aboveFloor

theorem covered_generation_quarantines
    (floor : GcFloor)
    (entry : RegistryEntry)
    (covered : entry.generation ≤ floor) :
    ImportAdmission floor entry .quarantine := by
  exact ImportAdmission.covered entry covered

theorem covered_generation_cannot_resume
    (floor : GcFloor)
    (entry : RegistryEntry)
    (covered : entry.generation ≤ floor) :
    ¬ ImportAdmission floor entry .resume := by
  intro admission
  cases admission with
  | current aboveFloor =>
      exact (Nat.not_lt_of_ge covered) aboveFloor

theorem canonical_retired_compaction_constructible
    (key : KeyId)
    (generation floor : Nat)
    (covered : generation ≤ floor) :
    let before : MultiKeySnapshot := {
      snapshotRevision := 0
      manifest := [key]
      entries := [
        { key := key, generation := generation, state := .retired }
      ]
      gcFloor := floor
    }
    let after : MultiKeySnapshot := {
      snapshotRevision := 1
      manifest := [key]
      entries := []
      gcFloor := floor
    }
    let choice : CanonicalMultiKeyChoice := fun _ => after
    CanonicalCompactionCas choice 0
      { revision := 0, snapshot := before }
      { revision := 1, snapshot := after } := by
  exact CanonicalCompactionCas.commit 0 _
    (SafeMultiKeyCompaction.compact
      0 [key]
      [{ key := key, generation := generation, state := .retired }]
      [] floor floor (Nat.le_refl floor)
      (covered_retired_entry_can_compact floor key generation covered)
      FloorSafeForEntries.nil)

theorem stale_compaction_revision_cannot_commit
    (choice : CanonicalMultiKeyChoice)
    (expected current : MultiKeyLedgerRevision)
    (snapshot : MultiKeySnapshot)
    (stale : expected ≠ current) :
    ¬ ∃ after,
      CanonicalCompactionCas choice expected
        { revision := current, snapshot := snapshot }
        after := by
  intro witness
  rcases witness with ⟨after, commit⟩
  cases commit
  exact stale rfl

theorem canonical_same_head_compactions_are_unique
    (choice : CanonicalMultiKeyChoice)
    (expected : MultiKeyLedgerRevision)
    (before left right : MultiKeyLedgerHead)
    (leftCommit : CanonicalCompactionCas choice expected before left)
    (rightCommit : CanonicalCompactionCas choice expected before right) :
    left = right := by
  cases leftCommit
  cases rightCommit
  rfl

theorem canonical_compaction_advances_ledger_revision
    (choice : CanonicalMultiKeyChoice)
    (revision : MultiKeyLedgerRevision)
    (before after : MultiKeyLedgerHead)
    (commit : CanonicalCompactionCas choice revision before after) :
    after.revision = revision + 1 := by
  cases commit
  rfl

theorem present_accepted_entry_can_finalize
    (ledgerRevision : MultiKeyLedgerRevision)
    (snapshotRevision : MultiKeySnapshotRevision)
    (manifest : List KeyId)
    (key : KeyId)
    (generation : EntryGeneration)
    (receipt : ReceiptId)
    (rest : List RegistryEntry)
    (gcFloor : GcFloor) :
    MultiKeyDelivered
      {
        revision := ledgerRevision
        snapshot := {
          snapshotRevision := snapshotRevision
          manifest := manifest
          entries :=
            { key := key, generation := generation,
              state := .accepted receipt } :: rest
          gcFloor := gcFloor
        }
      }
      key receipt := by
  exact MultiKeyDelivered.verified _ key receipt
    (ContainsAccepted.head key generation receipt rest)

theorem missing_accepted_entry_cannot_finalize
    (ledgerRevision : MultiKeyLedgerRevision)
    (snapshotRevision : MultiKeySnapshotRevision)
    (manifest : List KeyId)
    (key : KeyId)
    (receipt : ReceiptId)
    (gcFloor : GcFloor) :
    ¬ MultiKeyDelivered
      {
        revision := ledgerRevision
        snapshot := {
          snapshotRevision := snapshotRevision
          manifest := manifest
          entries := []
          gcFloor := gcFloor
        }
      }
      key receipt := by
  intro delivered
  cases delivered with
  | verified contains => cases contains

end ASPProof.AgentSessionHostRegistryCompaction
