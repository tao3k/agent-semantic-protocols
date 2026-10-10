-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.AgentSessionHostRegistryAuthenticatedProjection

namespace ASPProof.AgentSessionHostRegistryMerklePathConformance

open ASPProof.AgentSessionHostRegistryCompaction
open ASPProof.AgentSessionHostRegistryManifest
open ASPProof.AgentSessionHostRegistryAuthenticatedProjection

abbrev MerkleDigest := Nat
abbrev BranchHasher := MerkleDigest → MerkleDigest → MerkleDigest

inductive MerkleDomain where
  | leaf
  | branch
  | empty
  deriving DecidableEq, Repr

structure CanonicalLeafInput where
  domain : MerkleDomain
  schemaVersion : Nat
  serializationVersion : Nat
  key : KeyId
  generation : EntryGeneration
  disposition : ManifestDisposition
  deriving DecidableEq, Repr

abbrev LeafHasher := CanonicalLeafInput → MerkleDigest

def canonicalLeafInput (record : ManifestRecord) : CanonicalLeafInput :=
  {
    domain := .leaf
    schemaVersion := 1
    serializationVersion := 1
    key := record.key
    generation := record.generation
    disposition := record.disposition
  }

inductive SiblingSide where
  | left
  | right
  deriving DecidableEq, Repr

structure PathStep where
  side : SiblingSide
  sibling : MerkleDigest
  deriving DecidableEq, Repr

def applyPathStep
    (branch : BranchHasher)
    (current : MerkleDigest)
    (step : PathStep) : MerkleDigest :=
  match step.side with
  | .left => branch step.sibling current
  | .right => branch current step.sibling

def recomputeRoot
    (branch : BranchHasher)
    (leaf : MerkleDigest)
    (steps : List PathStep) : MerkleDigest :=
  steps.foldl (applyPathStep branch) leaf

structure InclusionPacket where
  record : ManifestRecord
  leafDigest : MerkleDigest
  steps : List PathStep
  rootDepth : Nat
  claimedRoot : MerkleDigest
  deriving Repr

def ExecutablePathAccepted
    (leafHash : LeafHasher)
    (branch : BranchHasher)
    (packet : InclusionPacket) : Prop :=
  packet.leafDigest = leafHash (canonicalLeafInput packet.record) ∧
  recomputeRoot branch packet.leafDigest packet.steps = packet.claimedRoot ∧
  packet.steps.length ≤ packet.rootDepth

def DirectionSensitive (branch : BranchHasher) : Prop :=
  ∀ left right, left ≠ right → branch left right ≠ branch right left

def naiveLeafDigest (key generation : Nat) : Nat := key + generation

def wrongRootPacket
    (leafHash : LeafHasher)
    (branch : BranchHasher)
    (record : ManifestRecord)
    (steps : List PathStep)
    (rootDepth : Nat) : InclusionPacket :=
  let leaf := leafHash (canonicalLeafInput record)
  {
    record := record
    leafDigest := leaf
    steps := steps
    rootDepth := rootDepth
    claimedRoot := recomputeRoot branch leaf steps + 1
  }

def unboundedPacket
    (leafHash : LeafHasher)
    (branch : BranchHasher)
    (record : ManifestRecord)
    (step : PathStep) : InclusionPacket :=
  let leaf := leafHash (canonicalLeafInput record)
  {
    record := record
    leafDigest := leaf
    steps := [step]
    rootDepth := 0
    claimedRoot := recomputeRoot branch leaf [step]
  }

structure InclusionConformance
    (commitment : ManifestCommitment)
    (leafHash : LeafHasher)
    (branch : BranchHasher)
    (snapshot : ManifestSnapshot)
    (root : ManifestRootDigest)
    (packet : InclusionPacket) : Prop where
  pathAccepted : ExecutablePathAccepted leafHash branch packet
  rootMatches : packet.claimedRoot = root
  semanticInclusion :
    AuthenticatedInclusion commitment snapshot root packet.record

structure ExclusionPacket where
  lowerPacket : Option InclusionPacket
  upperPacket : Option InclusionPacket
  deriving Repr

structure ExclusionConformance
    (commitment : ManifestCommitment)
    (leafHash : LeafHasher)
    (branch : BranchHasher)
    (snapshot : ManifestSnapshot)
    (root : ManifestRootDigest)
    (key : KeyId)
    (gap : ExclusionPacket) : Prop where
  lowerAccepted : ∀ packet, gap.lowerPacket = some packet →
    ExecutablePathAccepted leafHash branch packet ∧ packet.claimedRoot = root
  upperAccepted : ∀ packet, gap.upperPacket = some packet →
    ExecutablePathAccepted leafHash branch packet ∧ packet.claimedRoot = root
  semanticExclusion : AuthenticatedExclusion commitment snapshot root key

structure ForgedGapClaim where
  root : ManifestRootDigest
  key : KeyId
  lowerKey : Option KeyId
  upperKey : Option KeyId

abbrev RuntimePathVerifier := InclusionPacket → Bool

def RuntimeVerifierConforms
    (verifier : RuntimePathVerifier)
    (leafHash : LeafHasher)
    (branch : BranchHasher) : Prop :=
  ∀ packet,
    verifier packet = true ↔ ExecutablePathAccepted leafHash branch packet

def alwaysAcceptVerifier : RuntimePathVerifier := fun _ => true

def pathTokenCost (wordsPerStep : Nat) (packet : InclusionPacket) : Nat :=
  packet.steps.length * wordsPerStep

def oddDuplicateRoot
    (branch : BranchHasher)
    (digest : MerkleDigest) : MerkleDigest :=
  branch digest digest

theorem canonical_leaf_domain_is_leaf
    (record : ManifestRecord) :
    (canonicalLeafInput record).domain = .leaf := by
  rfl

theorem canonical_leaf_binds_record
    (record : ManifestRecord) :
    (canonicalLeafInput record).key = record.key ∧
    (canonicalLeafInput record).generation = record.generation ∧
    (canonicalLeafInput record).disposition = record.disposition := by
  exact ⟨rfl, rfl, rfl⟩

theorem merkle_domains_are_separated :
    MerkleDomain.leaf ≠ MerkleDomain.branch ∧
    MerkleDomain.branch ≠ MerkleDomain.empty := by
  decide

theorem naive_leaf_encoding_is_ambiguous :
    naiveLeafDigest 1 2 = naiveLeafDigest 2 1 ∧
    (1, 2) ≠ (2, 1) := by
  decide

theorem empty_path_preserves_leaf
    (branch : BranchHasher)
    (leaf : MerkleDigest) :
    recomputeRoot branch leaf [] = leaf := by
  rfl

theorem left_sibling_order_is_executable
    (branch : BranchHasher)
    (leaf sibling : MerkleDigest) :
    recomputeRoot branch leaf [{ side := .left, sibling := sibling }] =
      branch sibling leaf := by
  rfl

theorem right_sibling_order_is_executable
    (branch : BranchHasher)
    (leaf sibling : MerkleDigest) :
    recomputeRoot branch leaf [{ side := .right, sibling := sibling }] =
      branch leaf sibling := by
  rfl

theorem flipping_direction_changes_one_step_root
    (branch : BranchHasher)
    (sensitive : DirectionSensitive branch)
    (leaf sibling : MerkleDigest)
    (different : leaf ≠ sibling) :
    recomputeRoot branch leaf [{ side := .left, sibling := sibling }] ≠
      recomputeRoot branch leaf [{ side := .right, sibling := sibling }] := by
  exact sensitive sibling leaf different.symm

theorem accepted_path_has_exact_leaf
    {leafHash : LeafHasher}
    {branch : BranchHasher}
    {packet : InclusionPacket}
    (accepted : ExecutablePathAccepted leafHash branch packet) :
    packet.leafDigest = leafHash (canonicalLeafInput packet.record) := by
  exact accepted.1

theorem accepted_path_recomputes_claimed_root
    {leafHash : LeafHasher}
    {branch : BranchHasher}
    {packet : InclusionPacket}
    (accepted : ExecutablePathAccepted leafHash branch packet) :
    recomputeRoot branch packet.leafDigest packet.steps = packet.claimedRoot := by
  exact accepted.2.1

theorem accepted_path_is_depth_bounded
    {leafHash : LeafHasher}
    {branch : BranchHasher}
    {packet : InclusionPacket}
    (accepted : ExecutablePathAccepted leafHash branch packet) :
    packet.steps.length ≤ packet.rootDepth := by
  exact accepted.2.2

theorem wrong_root_packet_is_rejected
    (leafHash : LeafHasher)
    (branch : BranchHasher)
    (record : ManifestRecord)
    (steps : List PathStep)
    (rootDepth : Nat) :
    ¬ ExecutablePathAccepted leafHash branch
      (wrongRootPacket leafHash branch record steps rootDepth) := by
  intro accepted
  exact (Nat.ne_of_lt (Nat.lt_succ_self _)) accepted.2.1

theorem unbounded_packet_is_rejected
    (leafHash : LeafHasher)
    (branch : BranchHasher)
    (record : ManifestRecord)
    (step : PathStep) :
    ¬ ExecutablePathAccepted leafHash branch
      (unboundedPacket leafHash branch record step) := by
  intro accepted
  exact Nat.not_succ_le_zero 0 accepted.2.2

theorem inclusion_conformance_implies_semantic_member
    {commitment : ManifestCommitment}
    {leafHash : LeafHasher}
    {branch : BranchHasher}
    {snapshot : ManifestSnapshot}
    {root : ManifestRootDigest}
    {packet : InclusionPacket}
    (conforms : InclusionConformance commitment leafHash branch
      snapshot root packet) :
    packet.record ∈ snapshot.records := by
  exact authenticated_inclusion_is_member conforms.semanticInclusion

theorem inclusion_conformance_binds_same_root
    {commitment : ManifestCommitment}
    {leafHash : LeafHasher}
    {branch : BranchHasher}
    {snapshot : ManifestSnapshot}
    {root : ManifestRootDigest}
    {packet : InclusionPacket}
    (conforms : InclusionConformance commitment leafHash branch
      snapshot root packet) :
    packet.claimedRoot = root := by
  exact conforms.rootMatches

theorem exclusion_conformance_implies_semantic_absence
    {commitment : ManifestCommitment}
    {leafHash : LeafHasher}
    {branch : BranchHasher}
    {snapshot : ManifestSnapshot}
    {root : ManifestRootDigest}
    {key : KeyId}
    {gap : ExclusionPacket}
    (conforms : ExclusionConformance commitment leafHash branch
      snapshot root key gap) :
    KeyAbsent key snapshot.records := by
  exact authenticated_exclusion_is_absent conforms.semanticExclusion

theorem forged_gap_claim_constructible
    (root : ManifestRootDigest)
    (key : KeyId) :
    ∃ claim : ForgedGapClaim,
      claim.root = root ∧ claim.key = key := by
  exact ⟨⟨root, key, none, none⟩, rfl, rfl⟩

theorem conforming_verifier_is_sound
    {verifier : RuntimePathVerifier}
    {leafHash : LeafHasher}
    {branch : BranchHasher}
    (conforms : RuntimeVerifierConforms verifier leafHash branch)
    (packet : InclusionPacket)
    (accepted : verifier packet = true) :
    ExecutablePathAccepted leafHash branch packet := by
  exact (conforms packet).mp accepted

theorem conforming_verifier_is_complete
    {verifier : RuntimePathVerifier}
    {leafHash : LeafHasher}
    {branch : BranchHasher}
    (conforms : RuntimeVerifierConforms verifier leafHash branch)
    (packet : InclusionPacket)
    (accepted : ExecutablePathAccepted leafHash branch packet) :
    verifier packet = true := by
  exact (conforms packet).mpr accepted

theorem always_accept_verifier_is_not_conforming
    (leafHash : LeafHasher)
    (branch : BranchHasher)
    (record : ManifestRecord)
    (steps : List PathStep)
    (rootDepth : Nat) :
    ¬ RuntimeVerifierConforms alwaysAcceptVerifier leafHash branch := by
  intro conforms
  have accepted := (conforms
    (wrongRootPacket leafHash branch record steps rootDepth)).mp rfl
  exact wrong_root_packet_is_rejected
    leafHash branch record steps rootDepth accepted

theorem accepted_path_token_cost_is_bounded
    {leafHash : LeafHasher}
    {branch : BranchHasher}
    {packet : InclusionPacket}
    (accepted : ExecutablePathAccepted leafHash branch packet)
    (wordsPerStep : Nat) :
    pathTokenCost wordsPerStep packet ≤ packet.rootDepth * wordsPerStep := by
  exact Nat.mul_le_mul_right wordsPerStep accepted.2.2

theorem odd_node_duplication_is_ordered
    (branch : BranchHasher)
    (digest : MerkleDigest) :
    oddDuplicateRoot branch digest = branch digest digest := by
  rfl

end ASPProof.AgentSessionHostRegistryMerklePathConformance
