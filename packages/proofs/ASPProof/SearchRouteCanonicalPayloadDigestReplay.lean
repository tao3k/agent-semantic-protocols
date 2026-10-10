-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteShortcutEnvelopeAdmission

namespace ASPProof.SearchRouteCanonicalPayloadDigestReplay

open SearchRouteShortcutCommitmentReplay
open SearchRouteShortcutEnvelopeAdmission

structure CanonicalShortcutPayload where
  schemaVersion : Nat
  hashAlgorithmId : Nat
  commitment : ShortcutCommitment
  directDominanceClaimed : Bool

structure CanonicalWordSequence where
  schemaVersion : Nat
  hashAlgorithmId : Nat
  candidateUniverseDigest : Nat
  startCanonicalRouteId : Nat
  endpointCanonicalRouteId : Nat
  costSemanticsDigest : Nat
  selectionPolicyDigest : Nat
  chainLength : Nat
  explicitChainAuditDigest : Nat
  theoremFamilyDigest : Nat
  auditReceiptDigest : Nat
  directDominanceClaimed : Bool

def canonicalEncode
    (payload : CanonicalShortcutPayload) : CanonicalWordSequence := {
  schemaVersion := payload.schemaVersion
  hashAlgorithmId := payload.hashAlgorithmId
  candidateUniverseDigest :=
    payload.commitment.candidateUniverseDigest
  startCanonicalRouteId :=
    payload.commitment.startCanonicalRouteId
  endpointCanonicalRouteId :=
    payload.commitment.endpointCanonicalRouteId
  costSemanticsDigest :=
    payload.commitment.costSemanticsDigest
  selectionPolicyDigest :=
    payload.commitment.selectionPolicyDigest
  chainLength := payload.commitment.chainLength
  explicitChainAuditDigest :=
    payload.commitment.explicitChainAuditDigest
  theoremFamilyDigest :=
    payload.commitment.theoremFamilyDigest
  auditReceiptDigest :=
    payload.commitment.auditReceiptDigest
  directDominanceClaimed := payload.directDominanceClaimed
}

def canonicalPayloadOfRaw
    (raw : RawShortcutEnvelope) : CanonicalShortcutPayload := {
  schemaVersion := raw.schemaVersion
  hashAlgorithmId := raw.hashAlgorithmId
  commitment := raw.commitment
  directDominanceClaimed := raw.claimedDirectDominance
}

theorem canonical_encoding_is_injective
    (left right : CanonicalShortcutPayload)
    (encoded : canonicalEncode left = canonicalEncode right) :
    left = right := by
  cases left with
  | mk leftSchema leftHash leftCommitment leftDominance =>
    cases right with
    | mk rightSchema rightHash rightCommitment rightDominance =>
      cases leftCommitment with
      | mk leftUniverse leftStart leftEndpoint leftCost
          leftSelection leftLength leftChainAudit
          leftTheorem leftAudit =>
        cases rightCommitment with
        | mk rightUniverse rightStart rightEndpoint rightCost
            rightSelection rightLength rightChainAudit
            rightTheorem rightAudit =>
          cases encoded
          rfl

theorem full_field_replay_needs_no_digest_assumption
    (expected actual : CanonicalShortcutPayload)
    (sameEncoding :
      canonicalEncode expected = canonicalEncode actual) :
    expected = actual :=
  canonical_encoding_is_injective expected actual sameEncoding

theorem schema_version_is_inside_canonical_preimage
    (left right : CanonicalShortcutPayload)
    (different : left.schemaVersion ≠ right.schemaVersion) :
    canonicalEncode left ≠ canonicalEncode right := by
  intro sameEncoding
  have payloadEqual :=
    canonical_encoding_is_injective left right sameEncoding
  exact different
    (congrArg CanonicalShortcutPayload.schemaVersion payloadEqual)

theorem hash_algorithm_is_inside_canonical_preimage
    (left right : CanonicalShortcutPayload)
    (different : left.hashAlgorithmId ≠ right.hashAlgorithmId) :
    canonicalEncode left ≠ canonicalEncode right := by
  intro sameEncoding
  have payloadEqual :=
    canonical_encoding_is_injective left right sameEncoding
  exact different
    (congrArg CanonicalShortcutPayload.hashAlgorithmId payloadEqual)

abbrev DigestFunction := CanonicalWordSequence → Nat

structure CollisionFreeOnAdmitted
    (digest : DigestFunction)
    (admitted : CanonicalShortcutPayload → Prop) : Prop where
  resolve :
    ∀ left right,
      admitted left →
      admitted right →
      digest (canonicalEncode left) =
        digest (canonicalEncode right) →
      canonicalEncode left = canonicalEncode right

theorem admitted_digest_equality_implies_payload_equality
    (digest : DigestFunction)
    (admitted : CanonicalShortcutPayload → Prop)
    (collisionFree : CollisionFreeOnAdmitted digest admitted)
    (left right : CanonicalShortcutPayload)
    (leftAdmitted : admitted left)
    (rightAdmitted : admitted right)
    (digestEqual :
      digest (canonicalEncode left) =
        digest (canonicalEncode right)) :
    left = right := by
  apply canonical_encoding_is_injective
  exact collisionFree.resolve
    left right leftAdmitted rightAdmitted digestEqual

def zeroCommitment : ShortcutCommitment := {
  candidateUniverseDigest := 0
  startCanonicalRouteId := 0
  endpointCanonicalRouteId := 0
  costSemanticsDigest := 0
  selectionPolicyDigest := 0
  chainLength := 0
  explicitChainAuditDigest := 0
  theoremFamilyDigest := 0
  auditReceiptDigest := 0
}

def schemaOnePayload : CanonicalShortcutPayload := {
  schemaVersion := 1
  hashAlgorithmId := 0
  commitment := zeroCommitment
  directDominanceClaimed := false
}

def schemaTwoPayload : CanonicalShortcutPayload := {
  schemaVersion := 2
  hashAlgorithmId := 0
  commitment := zeroCommitment
  directDominanceClaimed := false
}

def constantDigest (_words : CanonicalWordSequence) : Nat :=
  0

theorem constant_digest_collides :
    constantDigest (canonicalEncode schemaOnePayload) =
      constantDigest (canonicalEncode schemaTwoPayload) := by
  rfl

theorem collision_payloads_are_distinct :
    schemaOnePayload ≠ schemaTwoPayload := by
  intro equalPayloads
  have schemaEqual :=
    congrArg CanonicalShortcutPayload.schemaVersion equalPayloads
  change (1 : Nat) = 2 at schemaEqual
  have zeroEqualsOne : (0 : Nat) = 1 :=
    Nat.succ.inj schemaEqual
  exact Nat.zero_ne_one zeroEqualsOne

theorem digest_equality_alone_does_not_imply_payload_equality :
    ∃ digest : DigestFunction,
      ∃ left right : CanonicalShortcutPayload,
        digest (canonicalEncode left) =
          digest (canonicalEncode right) ∧
        left ≠ right := by
  exact ⟨
    constantDigest,
    schemaOnePayload,
    schemaTwoPayload,
    constant_digest_collides,
    collision_payloads_are_distinct
  ⟩

def fullFieldReplayTokens : Nat :=
  12

def digestCompressedReplayTokens : Nat :=
  3

theorem digest_compressed_replay_is_constant_shape :
    digestCompressedReplayTokens = 3 := by
  rfl

theorem digest_compressed_projection_is_smaller :
    digestCompressedReplayTokens < fullFieldReplayTokens := by
  decide

end ASPProof.SearchRouteCanonicalPayloadDigestReplay
