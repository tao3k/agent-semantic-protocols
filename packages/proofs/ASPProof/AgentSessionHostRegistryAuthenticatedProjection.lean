-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.AgentSessionHostRegistryManifest

namespace ASPProof.AgentSessionHostRegistryAuthenticatedProjection

open ASPProof.AgentSessionHostRegistryCompaction
open ASPProof.AgentSessionHostRegistryManifest

abbrev ManifestRootDigest := Nat
abbrev ManifestCommitment := CanonicalManifestInput → ManifestRootDigest

def rootFor
    (commitment : ManifestCommitment)
    (snapshot : ManifestSnapshot) : ManifestRootDigest :=
  commitment (canonicalManifestInput snapshot)

inductive AuthenticatedManifest
    (commitment : ManifestCommitment) :
    ManifestSnapshot → ManifestRootDigest → Prop where
  | certified
      (snapshot : ManifestSnapshot)
      (canonical : CanonicalManifest snapshot) :
      AuthenticatedManifest commitment snapshot
        (rootFor commitment snapshot)

def KeyAbsent (key : KeyId) (records : List ManifestRecord) : Prop :=
  ∀ record, record ∈ records → record.key ≠ key

inductive AuthenticatedInclusion
    (commitment : ManifestCommitment)
    (snapshot : ManifestSnapshot)
    (root : ManifestRootDigest)
    (record : ManifestRecord) : Prop where
  | certified
      (authenticated : AuthenticatedManifest commitment snapshot root)
      (member : record ∈ snapshot.records) :
      AuthenticatedInclusion commitment snapshot root record

inductive AuthenticatedExclusion
    (commitment : ManifestCommitment)
    (snapshot : ManifestSnapshot)
    (root : ManifestRootDigest)
    (key : KeyId) : Prop where
  | certified
      (authenticated : AuthenticatedManifest commitment snapshot root)
      (absent : KeyAbsent key snapshot.records) :
      AuthenticatedExclusion commitment snapshot root key

structure UnboundInclusionClaim where
  root : ManifestRootDigest
  record : ManifestRecord

structure UnboundExclusionClaim where
  root : ManifestRootDigest
  key : KeyId

structure DigestCollision (commitment : ManifestCommitment) where
  left : CanonicalManifestInput
  right : CanonicalManifestInput
  inputsDiffer : left ≠ right
  digestEqual : commitment left = commitment right

def TombstoneAt
    (key : KeyId)
    (generation : EntryGeneration)
    (snapshot : ManifestSnapshot) : Prop :=
  ({ key := key, generation := generation,
     disposition := .tombstoned } : ManifestRecord) ∈ snapshot.records

def OtherRecordsPreserved
    (updatedKey : KeyId)
    (before after : ManifestSnapshot) : Prop :=
  ∀ record, record.key ≠ updatedKey →
    (record ∈ before.records ↔ record ∈ after.records)

def OtherEntriesPreserved
    (updatedKey : KeyId)
    (before after : ManifestSnapshot) : Prop :=
  ∀ entry, entry.key ≠ updatedKey →
    (entry ∈ before.entries ↔ entry ∈ after.entries)

def CoveredTombstonesPreserved
    (before after : ManifestSnapshot) : Prop :=
  ∀ key generation,
    TombstoneAt key generation before →
    generation ≤ before.gcFloor →
    TombstoneAt key generation after

structure AuthenticatedManifestDelta
    (commitment : ManifestCommitment) where
  updatedKey : KeyId
  before : ManifestSnapshot
  after : ManifestSnapshot
  baseRoot : ManifestRootDigest
  nextRoot : ManifestRootDigest
  beforeAuthenticated : AuthenticatedManifest commitment before baseRoot
  afterAuthenticated : AuthenticatedManifest commitment after nextRoot
  revisionAdvances : after.snapshotRevision = before.snapshotRevision + 1
  semanticInputChanges :
    canonicalManifestInput before ≠ canonicalManifestInput after
  rootChanges : baseRoot ≠ nextRoot
  floorMonotone : before.gcFloor ≤ after.gcFloor
  otherRecordsPreserved : OtherRecordsPreserved updatedKey before after
  otherEntriesPreserved : OtherEntriesPreserved updatedKey before after
  coveredTombstonesPreserved : CoveredTombstonesPreserved before after

inductive NaiveTombstoneResurrection :
    ManifestSnapshot → ManifestSnapshot → Prop where
  | replace
      (snapshotRevision : MultiKeySnapshotRevision)
      (key : KeyId)
      (generation : EntryGeneration) :
      NaiveTombstoneResurrection
        {
          snapshotRevision := snapshotRevision
          records := [{ key := key, generation := generation,
                        disposition := .tombstoned }]
          entries := []
          gcFloor := generation
        }
        {
          snapshotRevision := snapshotRevision + 1
          records := [{ key := key, generation := generation,
                        disposition := .live }]
          entries := [{ key := key, generation := generation,
                        state := .empty }]
          gcFloor := generation
        }

structure StaleBaseClaim where
  requestedBaseRoot : ManifestRootDigest
  currentRoot : ManifestRootDigest
  stale : requestedBaseRoot ≠ currentRoot

structure AuthenticatedProjectionHead where
  revision : MultiKeyLedgerRevision
  snapshot : ManifestSnapshot
  root : ManifestRootDigest

abbrev AuthenticatedProjectionChoice :=
  MultiKeyLedgerRevision → ManifestSnapshot

inductive AuthenticatedProjectionCas
    (commitment : ManifestCommitment)
    (choice : AuthenticatedProjectionChoice) :
    MultiKeyLedgerRevision →
    AuthenticatedProjectionHead →
    AuthenticatedProjectionHead → Prop where
  | publish
      (revision : MultiKeyLedgerRevision)
      (delta : AuthenticatedManifestDelta commitment)
      (chosen : delta.after = choice (revision + 1))
      (canonical : CanonicalManifest (choice (revision + 1))) :
      AuthenticatedProjectionCas commitment choice revision
        { revision := revision
          snapshot := delta.before
          root := delta.baseRoot }
        { revision := revision + 1
          snapshot := choice (revision + 1)
          root := rootFor commitment (choice (revision + 1)) }

theorem authenticated_manifest_is_canonical
    {commitment : ManifestCommitment}
    {snapshot : ManifestSnapshot}
    {root : ManifestRootDigest}
    (authenticated : AuthenticatedManifest commitment snapshot root) :
    CanonicalManifest snapshot := by
  cases authenticated
  assumption

theorem authenticated_manifest_root_is_exact
    {commitment : ManifestCommitment}
    {snapshot : ManifestSnapshot}
    {root : ManifestRootDigest}
    (authenticated : AuthenticatedManifest commitment snapshot root) :
    root = rootFor commitment snapshot := by
  cases authenticated
  rfl

theorem authenticated_inclusion_is_member
    {commitment : ManifestCommitment}
    {snapshot : ManifestSnapshot}
    {root : ManifestRootDigest}
    {record : ManifestRecord}
    (proof : AuthenticatedInclusion commitment snapshot root record) :
    record ∈ snapshot.records := by
  cases proof
  assumption

theorem authenticated_exclusion_is_absent
    {commitment : ManifestCommitment}
    {snapshot : ManifestSnapshot}
    {root : ManifestRootDigest}
    {key : KeyId}
    (proof : AuthenticatedExclusion commitment snapshot root key) :
    KeyAbsent key snapshot.records := by
  cases proof
  assumption

theorem authenticated_inclusion_exclusion_conflict_impossible
    {commitment : ManifestCommitment}
    {snapshot : ManifestSnapshot}
    {root : ManifestRootDigest}
    {record : ManifestRecord}
    (inclusion : AuthenticatedInclusion commitment snapshot root record)
    (exclusion : AuthenticatedExclusion commitment snapshot root record.key) :
    False := by
  exact (authenticated_exclusion_is_absent exclusion)
    record (authenticated_inclusion_is_member inclusion) rfl

theorem conflicting_unbound_claims_constructible
    (root : ManifestRootDigest)
    (record : ManifestRecord) :
    ∃ inclusion : UnboundInclusionClaim,
      ∃ exclusion : UnboundExclusionClaim,
        inclusion.root = root ∧
        inclusion.record = record ∧
        exclusion.root = root ∧
        exclusion.key = record.key := by
  exact ⟨⟨root, record⟩, ⟨root, record.key⟩, rfl, rfl, rfl, rfl⟩

theorem digest_collision_has_one_root
    {commitment : ManifestCommitment}
    (collision : DigestCollision commitment) :
    commitment collision.left = commitment collision.right := by
  exact collision.digestEqual

theorem injective_commitment_identifies_input
    {commitment : ManifestCommitment}
    (sound : Function.Injective commitment)
    (left right : ManifestSnapshot)
    (sameRoot : rootFor commitment left = rootFor commitment right) :
    canonicalManifestInput left = canonicalManifestInput right := by
  exact sound sameRoot

theorem authenticated_delta_base_root_is_exact
    {commitment : ManifestCommitment}
    (delta : AuthenticatedManifestDelta commitment) :
    delta.baseRoot = rootFor commitment delta.before := by
  exact authenticated_manifest_root_is_exact delta.beforeAuthenticated

theorem authenticated_delta_next_root_is_exact
    {commitment : ManifestCommitment}
    (delta : AuthenticatedManifestDelta commitment) :
    delta.nextRoot = rootFor commitment delta.after := by
  exact authenticated_manifest_root_is_exact delta.afterAuthenticated

theorem authenticated_delta_revision_advances
    {commitment : ManifestCommitment}
    (delta : AuthenticatedManifestDelta commitment) :
    delta.after.snapshotRevision = delta.before.snapshotRevision + 1 := by
  exact delta.revisionAdvances

theorem authenticated_delta_semantic_input_changes
    {commitment : ManifestCommitment}
    (delta : AuthenticatedManifestDelta commitment) :
    canonicalManifestInput delta.before ≠
      canonicalManifestInput delta.after := by
  exact delta.semanticInputChanges

theorem authenticated_delta_root_changes
    {commitment : ManifestCommitment}
    (delta : AuthenticatedManifestDelta commitment) :
    delta.baseRoot ≠ delta.nextRoot := by
  exact delta.rootChanges

theorem authenticated_delta_floor_is_monotone
    {commitment : ManifestCommitment}
    (delta : AuthenticatedManifestDelta commitment) :
    delta.before.gcFloor ≤ delta.after.gcFloor := by
  exact delta.floorMonotone

theorem authenticated_delta_preserves_other_records
    {commitment : ManifestCommitment}
    (delta : AuthenticatedManifestDelta commitment) :
    OtherRecordsPreserved delta.updatedKey delta.before delta.after := by
  exact delta.otherRecordsPreserved

theorem authenticated_delta_preserves_other_entries
    {commitment : ManifestCommitment}
    (delta : AuthenticatedManifestDelta commitment) :
    OtherEntriesPreserved delta.updatedKey delta.before delta.after := by
  exact delta.otherEntriesPreserved

theorem authenticated_delta_preserves_covered_tombstones
    {commitment : ManifestCommitment}
    (delta : AuthenticatedManifestDelta commitment) :
    CoveredTombstonesPreserved delta.before delta.after := by
  exact delta.coveredTombstonesPreserved

theorem naive_tombstone_resurrection_constructible
    (snapshotRevision : MultiKeySnapshotRevision)
    (key : KeyId)
    (generation : EntryGeneration) :
    NaiveTombstoneResurrection
      {
        snapshotRevision := snapshotRevision
        records := [{ key := key, generation := generation,
                      disposition := .tombstoned }]
        entries := []
        gcFloor := generation
      }
      {
        snapshotRevision := snapshotRevision + 1
        records := [{ key := key, generation := generation,
                      disposition := .live }]
        entries := [{ key := key, generation := generation,
                      state := .empty }]
        gcFloor := generation
      } := by
  exact NaiveTombstoneResurrection.replace snapshotRevision key generation

theorem naive_tombstone_resurrection_breaks_preservation
    (snapshotRevision : MultiKeySnapshotRevision)
    (key : KeyId)
    (generation : EntryGeneration) :
    ¬ CoveredTombstonesPreserved
      {
        snapshotRevision := snapshotRevision
        records := [{ key := key, generation := generation,
                      disposition := .tombstoned }]
        entries := []
        gcFloor := generation
      }
      {
        snapshotRevision := snapshotRevision + 1
        records := [{ key := key, generation := generation,
                      disposition := .live }]
        entries := [{ key := key, generation := generation,
                      state := .empty }]
        gcFloor := generation
      } := by
  intro preserved
  have tombstoneAfter := preserved key generation
    (List.Mem.head _) (Nat.le_refl generation)
  cases tombstoneAfter with
  | tail _ tailMember => cases tailMember

theorem stale_base_claim_constructible
    (requested current : ManifestRootDigest)
    (stale : requested ≠ current) :
    ∃ claim : StaleBaseClaim,
      claim.requestedBaseRoot = requested ∧
      claim.currentRoot = current := by
  exact ⟨⟨requested, current, stale⟩, rfl, rfl⟩

theorem stale_base_claim_cannot_match_current
    (claim : StaleBaseClaim) :
    claim.requestedBaseRoot ≠ claim.currentRoot := by
  exact claim.stale

theorem authenticated_projection_cas_after_is_authenticated
    {commitment : ManifestCommitment}
    {choice : AuthenticatedProjectionChoice}
    {revision : MultiKeyLedgerRevision}
    {before after : AuthenticatedProjectionHead}
    (published : AuthenticatedProjectionCas commitment choice
      revision before after) :
    AuthenticatedManifest commitment after.snapshot after.root := by
  cases published
  exact AuthenticatedManifest.certified _ (by assumption)

theorem authenticated_projection_cas_uses_current_revision
    {commitment : ManifestCommitment}
    {choice : AuthenticatedProjectionChoice}
    {suppliedRevision : MultiKeyLedgerRevision}
    {before after : AuthenticatedProjectionHead}
    (published : AuthenticatedProjectionCas commitment choice
      suppliedRevision before after) :
    before.revision = suppliedRevision := by
  cases published
  rfl

theorem authenticated_projection_cas_after_is_choice
    {commitment : ManifestCommitment}
    {choice : AuthenticatedProjectionChoice}
    {revision : MultiKeyLedgerRevision}
    {before after : AuthenticatedProjectionHead}
    (published : AuthenticatedProjectionCas commitment choice
      revision before after) :
    after = {
      revision := revision + 1
      snapshot := choice (revision + 1)
      root := rootFor commitment (choice (revision + 1))
    } := by
  cases published
  rfl

theorem authenticated_projection_same_head_is_unique
    {commitment : ManifestCommitment}
    {choice : AuthenticatedProjectionChoice}
    {revision : MultiKeyLedgerRevision}
    {before first second : AuthenticatedProjectionHead}
    (firstPublish : AuthenticatedProjectionCas commitment choice
      revision before first)
    (secondPublish : AuthenticatedProjectionCas commitment choice
      revision before second) :
    first = second := by
  exact (authenticated_projection_cas_after_is_choice firstPublish).trans
    (authenticated_projection_cas_after_is_choice secondPublish).symm

end ASPProof.AgentSessionHostRegistryAuthenticatedProjection
