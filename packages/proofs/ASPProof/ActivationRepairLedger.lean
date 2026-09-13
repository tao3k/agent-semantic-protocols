-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.ActivationRepairJournal

namespace ASPProof.ActivationRepairLedger

structure EntryKey where
  canonicalIdentity : Nat
  generation : Nat
  authorityRevision : Nat
  deriving Repr, DecidableEq

inductive EntryPhase where
  | committed
  | enqueued
  | acknowledged
  | tombstoned
  deriving Repr, DecidableEq

structure LedgerEntry where
  key : EntryKey
  phase : EntryPhase
  deriving Repr, DecidableEq

structure RepairLedger where
  canonicalIdentity : Nat
  activeGeneration : Nat
  authorityRevision : Nat
  gcFloor : Nat
  entries : List LedgerEntry
  acknowledgedKeys : List EntryKey
  deriving Repr, DecidableEq

structure SinkAcknowledgement where
  key : EntryKey
  deriving Repr, DecidableEq

def CurrentKey (ledger : RepairLedger) (key : EntryKey) : Prop :=
  key.canonicalIdentity = ledger.canonicalIdentity ∧
  key.generation = ledger.activeGeneration ∧
  key.authorityRevision = ledger.authorityRevision

instance currentKeyDecidable (ledger : RepairLedger) (key : EntryKey) :
    Decidable (CurrentKey ledger key) := by
  unfold CurrentKey
  infer_instance

def ReplayAdmissible (ledger : RepairLedger) (entry : LedgerEntry) : Prop :=
  entry ∈ ledger.entries ∧
  CurrentKey ledger entry.key ∧
  ledger.gcFloor < entry.key.generation ∧
  (entry.phase = .committed ∨ entry.phase = .enqueued)

instance replayAdmissibleDecidable
    (ledger : RepairLedger)
    (entry : LedgerEntry) :
    Decidable (ReplayAdmissible ledger entry) := by
  unfold ReplayAdmissible
  infer_instance

def AckAdmissible
    (ledger : RepairLedger)
    (acknowledgement : SinkAcknowledgement) : Prop :=
  ∃ entry,
    entry ∈ ledger.entries ∧
    entry.key = acknowledgement.key ∧
    CurrentKey ledger entry.key ∧
    entry.phase = .enqueued

instance ackAdmissibleDecidable
    (ledger : RepairLedger)
    (acknowledgement : SinkAcknowledgement) :
    Decidable (AckAdmissible ledger acknowledgement) := by
  unfold AckAdmissible
  infer_instance

def recordAcknowledgement
    (ledger : RepairLedger)
    (acknowledgement : SinkAcknowledgement)
    (_admissible : AckAdmissible ledger acknowledgement) : RepairLedger :=
  { ledger with
    acknowledgedKeys := acknowledgement.key :: ledger.acknowledgedKeys }

def ImportAdmissible (ledger : RepairLedger) (entry : LedgerEntry) : Prop :=
  ledger.gcFloor < entry.key.generation ∧
  entry.key.canonicalIdentity = ledger.canonicalIdentity ∧
  entry.key.generation ≤ ledger.activeGeneration

instance importAdmissibleDecidable
    (ledger : RepairLedger)
    (entry : LedgerEntry) :
    Decidable (ImportAdmissible ledger entry) := by
  unfold ImportAdmissible
  infer_instance

def CanCollect (ledger : RepairLedger) (entry : LedgerEntry) : Prop :=
  entry ∈ ledger.entries ∧
  entry.phase = .tombstoned ∧
  entry.key.generation ≤ ledger.gcFloor ∧
  entry.key.generation < ledger.activeGeneration ∧
  ∃ currentKey,
    currentKey ∈ ledger.acknowledgedKeys ∧
    CurrentKey ledger currentKey

instance canCollectDecidable
    (ledger : RepairLedger)
    (entry : LedgerEntry) :
    Decidable (CanCollect ledger entry) := by
  unfold CanCollect
  infer_instance

def compactEntry
    (ledger : RepairLedger)
    (entry : LedgerEntry)
    (_collectable : CanCollect ledger entry) : RepairLedger :=
  { ledger with entries := ledger.entries.filter (fun candidate => candidate != entry) }

def generationOneKey : EntryKey :=
  { canonicalIdentity := 1
    generation := 1
    authorityRevision := 7 }

def generationTwoKey : EntryKey :=
  { canonicalIdentity := 1
    generation := 2
    authorityRevision := 8 }

def oldTombstone : LedgerEntry :=
  { key := generationOneKey
    phase := .tombstoned }

def currentEnqueued : LedgerEntry :=
  { key := generationTwoKey
    phase := .enqueued }

def twoGenerationLedger : RepairLedger :=
  { canonicalIdentity := 1
    activeGeneration := 2
    authorityRevision := 8
    gcFloor := 1
    entries := [currentEnqueued, oldTombstone]
    acknowledgedKeys := [generationTwoKey] }

def unsafeCompactedLedger : RepairLedger :=
  { twoGenerationLedger with
    gcFloor := 0
    entries := [currentEnqueued] }

def oldAcknowledgement : SinkAcknowledgement :=
  { key := generationOneKey }

def currentAcknowledgement : SinkAcknowledgement :=
  { key := generationTwoKey }

theorem stale_generation_cannot_replay
    (older : entry.key.generation < ledger.activeGeneration) :
    ¬ ReplayAdmissible ledger entry := by
  intro admissible
  have current : entry.key.generation = ledger.activeGeneration :=
    admissible.2.1.2.1
  exact (Nat.ne_of_lt older) current

theorem stale_generation_cannot_ack
    (older : acknowledgement.key.generation < ledger.activeGeneration) :
    ¬ AckAdmissible ledger acknowledgement := by
  intro admissible
  rcases admissible with ⟨entry, _member, sameKey, current, _phase⟩
  have equalGeneration : acknowledgement.key.generation =
      ledger.activeGeneration := by
    rw [← sameKey]
    exact current.2.1
  exact (Nat.ne_of_lt older) equalGeneration

theorem current_ack_is_admissible :
    AckAdmissible twoGenerationLedger currentAcknowledgement := by
  decide

theorem old_ack_is_rejected :
    ¬ AckAdmissible twoGenerationLedger oldAcknowledgement := by
  decide

theorem acknowledgement_reordering_preserves_head
    (admissible : AckAdmissible ledger acknowledgement) :
    let recorded := recordAcknowledgement ledger acknowledgement admissible
    recorded.activeGeneration = ledger.activeGeneration ∧
    recorded.authorityRevision = ledger.authorityRevision ∧
    recorded.gcFloor = ledger.gcFloor := by
  exact ⟨rfl, rfl, rfl⟩

theorem old_tombstone_is_collectable :
    CanCollect twoGenerationLedger oldTombstone := by
  exact
    ⟨List.Mem.tail currentEnqueued (List.Mem.head []),
      rfl,
      Nat.le_refl 1,
      Nat.lt_succ_self 1,
      generationTwoKey,
      List.Mem.head [],
      ⟨rfl, rfl, rfl⟩⟩

theorem compaction_preserves_head_and_gc_floor
    (collectable : CanCollect ledger entry) :
    let compacted := compactEntry ledger entry collectable
    compacted.activeGeneration = ledger.activeGeneration ∧
    compacted.authorityRevision = ledger.authorityRevision ∧
    compacted.gcFloor = ledger.gcFloor := by
  exact ⟨rfl, rfl, rfl⟩

theorem gc_floor_blocks_old_generation_resurrection :
    ¬ ImportAdmissible twoGenerationLedger oldTombstone := by
  decide

theorem deleting_tombstone_without_advancing_floor_allows_resurrection :
    ImportAdmissible unsafeCompactedLedger oldTombstone := by
  decide

theorem current_entry_remains_replayable :
    ReplayAdmissible twoGenerationLedger currentEnqueued := by
  exact
    ⟨List.Mem.head [oldTombstone],
      ⟨rfl, rfl, rfl⟩,
      Nat.lt_succ_self 1,
      Or.inr rfl⟩

/-- Cost model for the agent-facing recovery projection. -/
structure RecoveryProjection where
  unresolvedEntries : Nat
  graphHops : Nat
  interactionRounds : Nat
  exposedTokens : Nat
  deriving Repr, DecidableEq

def CostDominates
    (better worse : RecoveryProjection) : Prop :=
  better.unresolvedEntries ≤ worse.unresolvedEntries ∧
  better.graphHops ≤ worse.graphHops ∧
  better.interactionRounds ≤ worse.interactionRounds ∧
  better.exposedTokens ≤ worse.exposedTokens

instance costDominatesDecidable
    (better worse : RecoveryProjection) :
    Decidable (CostDominates better worse) := by
  unfold CostDominates
  infer_instance

def fullLedgerRecovery : RecoveryProjection :=
  { unresolvedEntries := 2
    graphHops := 5
    interactionRounds := 4
    exposedTokens := 1200 }

def unresolvedFrontierRecovery : RecoveryProjection :=
  { unresolvedEntries := 1
    graphHops := 2
    interactionRounds := 1
    exposedTokens := 240 }

def TokenBound
    (projection : RecoveryProjection)
    (baseTokens tokensPerEntry : Nat) : Prop :=
  projection.exposedTokens ≤
    baseTokens + tokensPerEntry * projection.unresolvedEntries

instance tokenBoundDecidable
    (projection : RecoveryProjection)
    (baseTokens tokensPerEntry : Nat) :
    Decidable (TokenBound projection baseTokens tokensPerEntry) := by
  unfold TokenBound
  infer_instance

theorem unresolved_frontier_dominates_full_ledger_recovery :
    CostDominates unresolvedFrontierRecovery fullLedgerRecovery := by
  decide

theorem unresolved_frontier_strictly_reduces_rounds :
    unresolvedFrontierRecovery.interactionRounds <
      fullLedgerRecovery.interactionRounds := by
  decide

theorem unresolved_frontier_strictly_reduces_tokens :
    unresolvedFrontierRecovery.exposedTokens <
      fullLedgerRecovery.exposedTokens := by
  decide

theorem unresolved_frontier_satisfies_linear_token_bound :
    TokenBound unresolvedFrontierRecovery 40 200 := by
  decide

end ASPProof.ActivationRepairLedger
