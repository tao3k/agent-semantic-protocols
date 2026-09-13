-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.RuntimeOwnerIdentityJournal

abbrev Owner := Nat
abbrev Generation := Nat
abbrev Digest := Nat

inductive OwnerIdentity where
  | present (digest : Digest)
  | missing
  | mutating (mutationId : Nat)
  deriving DecidableEq

structure Journal where
  epoch : Nat
  baseGeneration : Generation
  entries : Owner → Option OwnerIdentity

def rebase (journal : Journal) (nextBase : Generation) : Journal :=
  if journal.baseGeneration = nextBase then
    journal
  else
    { epoch := journal.epoch + 1
      baseGeneration := nextBase
      entries := fun _ => none }

def publishDelta
    (journal : Journal)
    (owner : Owner)
    (identity : OwnerIdentity) : Journal :=
  { journal with
    epoch := journal.epoch + 1
    entries := fun candidate =>
      if candidate = owner then some identity else journal.entries candidate }

def current
    (journal : Journal)
    (base : Generation)
    (owner : Owner)
    (digest : Digest) : Bool :=
  if journal.baseGeneration != base then
    false
  else
    match journal.entries owner with
    | none => true
    | some (.present currentDigest) => currentDigest == digest
    | some .missing => false
    | some (.mutating _) => false

def fencedCurrent
    (journal : Journal)
    (mappedBase pointerBase : Generation)
    (owner : Owner)
    (digest : Digest) : Bool :=
  if mappedBase != pointerBase then
    false
  else
    current journal pointerBase owner digest

def refreshResident (resident published : Journal) : Journal :=
  if resident.epoch < published.epoch then published else resident

structure MutationReceipt where
  mutationId : Nat
  payloadDigest : Digest
  epoch : Nat
  deriving DecidableEq

def admitReplay
    (committed : MutationReceipt)
    (mutationId : Nat)
    (payloadDigest : Digest) : Option MutationReceipt :=
  if committed.mutationId = mutationId then
    if committed.payloadDigest = payloadDigest then
      some committed
    else
      none
  else
    some { mutationId, payloadDigest, epoch := committed.epoch + 1 }

inductive PublicationPhase where
  | stablePrevious
  | writing
  | stableNext

def readPhase
    (phase : PublicationPhase)
    (previous next : Journal) : Option Journal :=
  match phase with
  | .stablePrevious => some previous
  | .writing => none
  | .stableNext => some next

structure PointerLease where
  generation : Nat
  snapshot : Journal

def publishLease (current candidate : PointerLease) : PointerLease :=
  if current.generation < candidate.generation then candidate else current

structure PublishRecovery where
  generationCommitted : Bool
  committedBase : Generation
  journalBase : Generation

def recoverJournalBase (state : PublishRecovery) : Generation :=
  if state.generationCommitted then state.committedBase else state.journalBase

theorem same_base_rebase_preserves_delta
    (journal : Journal)
    (base : Generation)
    (h : journal.baseGeneration = base) :
    rebase journal base = journal := by
  simp [rebase, h]

theorem new_base_rebase_clears_overlay
    (journal : Journal)
    (base : Generation)
    (owner : Owner)
    (h : journal.baseGeneration ≠ base) :
    (rebase journal base).entries owner = none := by
  simp [rebase, h]

theorem tombstone_never_proves_current
    (journal : Journal)
    (base : Generation)
    (owner : Owner)
    (digest : Digest)
    (baseCovered : journal.baseGeneration = base)
    (tombstoned : journal.entries owner = some .missing) :
    current journal base owner digest = false := by
  simp [current, baseCovered, tombstoned]

theorem mutating_owner_never_proves_current
    (journal : Journal)
    (base : Generation)
    (owner : Owner)
    (digest : Digest)
    (mutationId : Nat)
    (baseCovered : journal.baseGeneration = base)
    (mutating : journal.entries owner = some (.mutating mutationId)) :
    current journal base owner digest = false := by
  simp [current, baseCovered, mutating]

theorem mismatched_digest_never_proves_current
    (journal : Journal)
    (base : Generation)
    (owner : Owner)
    (published requested : Digest)
    (baseCovered : journal.baseGeneration = base)
    (present : journal.entries owner = some (.present published))
    (different : published ≠ requested) :
    current journal base owner requested = false := by
  simp [current, baseCovered, present, different]

theorem cross_generation_evidence_never_proves_current
    (journal : Journal)
    (mappedBase pointerBase : Generation)
    (owner : Owner)
    (digest : Digest)
    (different : mappedBase ≠ pointerBase) :
    fencedCurrent journal mappedBase pointerBase owner digest = false := by
  simp [fencedCurrent, different]

theorem advanced_resident_observes_published_epoch
    (resident published : Journal)
    (advanced : resident.epoch < published.epoch) :
    refreshResident resident published = published := by
  simp [refreshResident, advanced]

theorem identical_mutation_replay_preserves_epoch
    (committed : MutationReceipt) :
    admitReplay committed committed.mutationId committed.payloadDigest =
      some committed := by
  simp [admitReplay]

theorem conflicting_mutation_replay_is_rejected
    (committed : MutationReceipt)
    (payloadDigest : Digest)
    (different : committed.payloadDigest ≠ payloadDigest) :
    admitReplay committed committed.mutationId payloadDigest = none := by
  simp [admitReplay, different]

theorem seqlock_read_is_previous_or_next_complete_epoch
    (phase : PublicationPhase)
    (previous next observed : Journal)
    (readable : readPhase phase previous next = some observed) :
    observed = previous ∨ observed = next := by
  cases phase <;> simp [readPhase] at readable
  · exact Or.inl readable.symm
  · exact Or.inr readable.symm

theorem concurrent_decoder_cannot_regress_lease
    (current candidate : PointerLease) :
    current.generation ≤ (publishLease current candidate).generation := by
  by_cases advanced : current.generation < candidate.generation
  · simp [publishLease, advanced, Nat.le_of_lt advanced]
  · simp [publishLease, advanced]

theorem committed_generation_is_startup_recovery_authority
    (state : PublishRecovery)
    (committed : state.generationCommitted = true) :
    recoverJournalBase state = state.committedBase := by
  simp [recoverJournalBase, committed]

theorem uncommitted_generation_cannot_advance_journal_base
    (state : PublishRecovery)
    (uncommitted : state.generationCommitted = false) :
    recoverJournalBase state = state.journalBase := by
  simp [recoverJournalBase, uncommitted]

end ASPProof.RuntimeOwnerIdentityJournal
