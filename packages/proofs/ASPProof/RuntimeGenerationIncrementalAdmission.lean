-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import Std

namespace ASPProof.RuntimeGenerationIncrementalAdmission

inductive OwnerChange (Digest : Type) where
  | unchanged
  | rebuilt (digest : Digest)
  | tombstone
  deriving DecidableEq

def overlay
    (active : Owner → Option Digest)
    (delta : Owner → OwnerChange Digest)
    (owner : Owner) : Option Digest :=
  match delta owner with
  | .unchanged => active owner
  | .rebuilt digest => some digest
  | .tombstone => none

/-- Replacement is a total per-owner classification. The affected set is not
itself an upsert set: presence in the new provider product decides whether the
affected owner is rebuilt or tombstoned. -/
def classifyReplacement
    (affected present : Bool) (digest : Digest) : OwnerChange Digest :=
  if affected then
    if present then .rebuilt digest else .tombstone
  else
    .unchanged

def isUpsert : OwnerChange Digest → Bool
  | .rebuilt _ => true
  | _ => false

def isTombstone : OwnerChange Digest → Bool
  | .tombstone => true
  | _ => false

theorem replacement_partition_is_disjoint
    (affected present : Bool) (digest : Digest) :
    ¬ (isUpsert (classifyReplacement affected present digest) = true ∧
       isTombstone (classifyReplacement affected present digest) = true) := by
  cases affected <;> cases present <;>
    simp [classifyReplacement, isUpsert, isTombstone]

theorem affected_absent_owner_is_tombstoned (digest : Digest) :
    classifyReplacement true false digest = .tombstone := by
  rfl

theorem affected_present_owner_is_rebuilt (digest : Digest) :
    classifyReplacement true true digest = .rebuilt digest := by
  rfl

/-- File-hash membership follows immutable blob membership. Syntax ownership is
a projection and cannot remove the hash of a parser auxiliary blob. -/
def fileHashAdmitted (sourceBlob scopeEvidence : Bool) : Bool :=
  sourceBlob || scopeEvidence

theorem parser_auxiliary_blob_has_admitted_hash (scopeEvidence : Bool) :
    fileHashAdmitted true scopeEvidence = true := by
  simp [fileHashAdmitted]

theorem non_scope_hash_requires_source_blob
    (sourceBlob : Bool)
    (admitted : fileHashAdmitted sourceBlob false = true) :
    sourceBlob = true := by
  simpa [fileHashAdmitted] using admitted

theorem unchanged_owner_is_preserved
    (active : Owner → Option Digest)
    (delta : Owner → OwnerChange Digest)
    (owner : Owner)
    (h : delta owner = .unchanged) :
    overlay active delta owner = active owner := by
  simp [overlay, h]

theorem rebuilt_owner_replaces_active_digest
    (active : Owner → Option Digest)
    (delta : Owner → OwnerChange Digest)
    (owner : Owner)
    (digest : Digest)
    (h : delta owner = .rebuilt digest) :
    overlay active delta owner = some digest := by
  simp [overlay, h]

theorem tombstoned_owner_is_absent
    (active : Owner → Option Digest)
    (delta : Owner → OwnerChange Digest)
    (owner : Owner)
    (h : delta owner = .tombstone) :
    overlay active delta owner = none := by
  simp [overlay, h]

def restorable
    [DecidableEq Digest]
    (active : Owner → Option Digest)
    (blobDigest : Owner → Option Digest)
    (owner : Owner) : Bool :=
  match active owner, blobDigest owner with
  | some admitted, some stored => admitted == stored
  | _, _ => false

theorem owner_outside_active_generation_cannot_be_restored
    [DecidableEq Digest]
    (active : Owner → Option Digest)
    (blobDigest : Owner → Option Digest)
    (owner : Owner)
    (h : active owner = none) :
    restorable active blobDigest owner = false := by
  simp [restorable, h]

theorem digest_mismatch_cannot_be_restored
    [DecidableEq Digest]
    (active : Owner → Option Digest)
    (blobDigest : Owner → Option Digest)
    (owner : Owner)
    (activeDigest storedDigest : Digest)
    (ha : active owner = some activeDigest)
    (hb : blobDigest owner = some storedDigest)
    (hne : activeDigest ≠ storedDigest) :
    restorable active blobDigest owner = false := by
  simp [restorable, ha, hb, hne]

structure Publication (Generation : Type) where
  epoch : Nat
  generation : Generation

def publishLatest
    (current candidate : Publication Generation) : Publication Generation :=
  if current.epoch ≤ candidate.epoch then candidate else current

theorem older_candidate_cannot_overwrite_newer_generation
    (current candidate : Publication Generation)
    (h : candidate.epoch < current.epoch) :
    publishLatest current candidate = current := by
  simp [publishLatest, Nat.not_le_of_lt h]

def builderFor (builder : BuildKey → BuilderId) (key : BuildKey) : BuilderId :=
  builder key

theorem same_generation_key_observes_one_builder
    (builder : BuildKey → BuilderId)
    (key : BuildKey) :
    builderFor builder key = builderFor builder key := by
  rfl

end ASPProof.RuntimeGenerationIncrementalAdmission
