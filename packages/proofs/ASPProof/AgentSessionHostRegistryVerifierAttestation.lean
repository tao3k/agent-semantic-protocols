import ASPProof.AgentSessionHostRegistryMerklePathConformance

namespace ASPProof.AgentSessionHostRegistryVerifierAttestation

open ASPProof.AgentSessionHostRegistryCompaction
open ASPProof.AgentSessionHostRegistryManifest
open ASPProof.AgentSessionHostRegistryAuthenticatedProjection
open ASPProof.AgentSessionHostRegistryMerklePathConformance

abbrev ByteString := List Nat

structure LengthPrefixedBytes where
  payload : ByteString
  encodedLength : Nat
  lengthExact : encodedLength = payload.length

def canonicalField (payload : ByteString) : LengthPrefixedBytes :=
  { payload := payload, encodedLength := payload.length, lengthExact := rfl }

structure CanonicalLeafFrame where
  domainTag : Nat
  schemaVersion : Nat
  serializationVersion : Nat
  key : LengthPrefixedBytes
  generation : LengthPrefixedBytes
  dispositionTag : Nat

def canonicalLeafFrame
    (key generation : ByteString)
    (dispositionTag : Nat) : CanonicalLeafFrame :=
  {
    domainTag := 1
    schemaVersion := 1
    serializationVersion := 1
    key := canonicalField key
    generation := canonicalField generation
    dispositionTag := dispositionTag
  }

def naiveConcatenate (left right : ByteString) : ByteString := left ++ right

structure RawPathEnvelope where
  domainTag : Nat
  declaredLength : Nat
  payload : ByteString

inductive MalformedEnvelope : RawPathEnvelope → Prop where
  | wrongDomain
      (raw : RawPathEnvelope)
      (wrong : raw.domainTag ≠ 1) :
      MalformedEnvelope raw
  | lengthMismatch
      (raw : RawPathEnvelope)
      (wrong : raw.declaredLength ≠ raw.payload.length) :
      MalformedEnvelope raw

inductive DecodeError where
  | invalidDomain
  | truncated
  | lengthMismatch
  | invalidDirection
  | trailingBytes
  deriving DecidableEq, Repr

abbrev PacketDecoder := RawPathEnvelope → Except DecodeError InclusionPacket

def DecoderTerminates (decoder : PacketDecoder) : Prop :=
  ∀ raw, ∃ result, decoder raw = result

def DecoderRejectsMalformed (decoder : PacketDecoder) : Prop :=
  ∀ raw, MalformedEnvelope raw →
    ∃ error, decoder raw = .error error

def alwaysDecode
    (packet : InclusionPacket) : PacketDecoder :=
  fun _ => .ok packet

abbrev DomainDigest := MerkleDomain → ByteString → MerkleDigest

structure DigestTestVector where
  domain : MerkleDomain
  input : ByteString
  expected : MerkleDigest
  deriving DecidableEq, Repr

def VectorPasses
    (digest : DomainDigest)
    (vector : DigestTestVector) : Prop :=
  digest vector.domain vector.input = vector.expected

def alwaysZeroDigest : DomainDigest := fun _ _ => 0

structure VerifierArtifactIdentity where
  algorithmId : Nat
  artifactDigest : Nat
  schemaDigest : Nat
  decoderDigest : Nat
  deriving DecidableEq, Repr

structure VerifierAttestationData where
  artifact : VerifierArtifactIdentity
  vectors : List DigestTestVector

structure VerifierAttestation
    (expectedArtifact : VerifierArtifactIdentity)
    (data : VerifierAttestationData)
    (digest : DomainDigest)
    (decoder : PacketDecoder)
    (verifier : RuntimePathVerifier)
    (leafHash : LeafHasher)
    (branch : BranchHasher) : Prop where
  artifactMatches : data.artifact = expectedArtifact
  decoderTerminates : DecoderTerminates decoder
  malformedRejected : DecoderRejectsMalformed decoder
  vectorsPass : ∀ vector, vector ∈ data.vectors → VectorPasses digest vector
  verifierConforms : RuntimeVerifierConforms verifier leafHash branch

structure AxleVerifierReceiptData where
  leanModuleDigest : Nat
  leanAuditDigest : Nat
  verifierArtifactDigest : Nat
  vectorSuiteDigest : Nat
  deriving DecidableEq, Repr

def AxleVerifierReceiptAccepted
    (expectedLeanModuleDigest : Nat)
    (expectedLeanAuditDigest : Nat)
    (expectedVerifierArtifactDigest : Nat)
    (expectedVectorSuiteDigest : Nat)
    (receipt : AxleVerifierReceiptData) : Prop :=
  receipt.leanModuleDigest = expectedLeanModuleDigest ∧
  receipt.leanAuditDigest = expectedLeanAuditDigest ∧
  receipt.verifierArtifactDigest = expectedVerifierArtifactDigest ∧
  receipt.vectorSuiteDigest = expectedVectorSuiteDigest

structure UnboundAxleReceipt where
  claimedPass : Bool
  noteDigest : Nat

theorem canonical_field_length_is_exact
    (payload : ByteString) :
    (canonicalField payload).encodedLength = payload.length := by
  rfl

theorem canonical_leaf_frame_has_leaf_domain
    (key generation : ByteString)
    (dispositionTag : Nat) :
    (canonicalLeafFrame key generation dispositionTag).domainTag = 1 := by
  rfl

theorem canonical_leaf_frame_binds_key_length
    (key generation : ByteString)
    (dispositionTag : Nat) :
    (canonicalLeafFrame key generation dispositionTag).key.encodedLength =
      key.length := by
  rfl

theorem canonical_leaf_frame_binds_generation_length
    (key generation : ByteString)
    (dispositionTag : Nat) :
    (canonicalLeafFrame key generation dispositionTag).generation.encodedLength =
      generation.length := by
  rfl

theorem naive_concatenation_is_ambiguous :
    naiveConcatenate [1] [2, 3] = naiveConcatenate [1, 2] [3] ∧
    ([1], [2, 3]) ≠ ([1, 2], [3]) := by
  decide

theorem equal_canonical_fields_have_equal_payloads
    (left right : ByteString)
    (equal : canonicalField left = canonicalField right) :
    left = right := by
  exact congrArg LengthPrefixedBytes.payload equal

theorem wrong_domain_is_malformed
    (declaredLength : Nat)
    (payload : ByteString) :
    MalformedEnvelope {
      domainTag := 0
      declaredLength := declaredLength
      payload := payload
    } := by
  exact MalformedEnvelope.wrongDomain _ Nat.zero_ne_one

theorem wrong_length_is_malformed
    (payload : ByteString) :
    MalformedEnvelope {
      domainTag := 1
      declaredLength := payload.length + 1
      payload := payload
    } := by
  exact MalformedEnvelope.lengthMismatch _
    (Nat.ne_of_gt (Nat.lt_succ_self payload.length))

theorem every_lean_decoder_terminates
    (decoder : PacketDecoder) :
    DecoderTerminates decoder := by
  intro raw
  exact ⟨decoder raw, rfl⟩

theorem rejecting_decoder_returns_error
    (decoder : PacketDecoder)
    (rejects : DecoderRejectsMalformed decoder)
    (raw : RawPathEnvelope)
    (malformed : MalformedEnvelope raw) :
    ∃ error, decoder raw = .error error := by
  exact rejects raw malformed

theorem always_decode_does_not_reject_malformed
    (packet : InclusionPacket) :
    ¬ DecoderRejectsMalformed (alwaysDecode packet) := by
  intro rejects
  have malformed : MalformedEnvelope {
      domainTag := 0, declaredLength := 0, payload := [] } :=
    MalformedEnvelope.wrongDomain _ (by decide)
  obtain ⟨error, impossible⟩ := rejects _ malformed
  cases impossible

theorem passing_vector_matches_expected
    (digest : DomainDigest)
    (vector : DigestTestVector)
    (passes : VectorPasses digest vector) :
    digest vector.domain vector.input = vector.expected := by
  exact passes

theorem always_zero_fails_nonzero_vector
    (domain : MerkleDomain)
    (input : ByteString) :
    ¬ VectorPasses alwaysZeroDigest {
      domain := domain, input := input, expected := 1 } := by
  intro passes
  exact Nat.zero_ne_one passes

theorem attestation_binds_artifact
    {expectedArtifact : VerifierArtifactIdentity}
    {data : VerifierAttestationData}
    {digest : DomainDigest}
    {decoder : PacketDecoder}
    {verifier : RuntimePathVerifier}
    {leafHash : LeafHasher}
    {branch : BranchHasher}
    (attested : VerifierAttestation expectedArtifact data digest decoder
      verifier leafHash branch) :
    data.artifact = expectedArtifact := by
  exact attested.artifactMatches

theorem attestation_rejects_malformed
    {expectedArtifact : VerifierArtifactIdentity}
    {data : VerifierAttestationData}
    {digest : DomainDigest}
    {decoder : PacketDecoder}
    {verifier : RuntimePathVerifier}
    {leafHash : LeafHasher}
    {branch : BranchHasher}
    (attested : VerifierAttestation expectedArtifact data digest decoder
      verifier leafHash branch) :
    DecoderRejectsMalformed decoder := by
  exact attested.malformedRejected

theorem attestation_passes_every_declared_vector
    {expectedArtifact : VerifierArtifactIdentity}
    {data : VerifierAttestationData}
    {digest : DomainDigest}
    {decoder : PacketDecoder}
    {verifier : RuntimePathVerifier}
    {leafHash : LeafHasher}
    {branch : BranchHasher}
    (attested : VerifierAttestation expectedArtifact data digest decoder
      verifier leafHash branch)
    (vector : DigestTestVector)
    (member : vector ∈ data.vectors) :
    VectorPasses digest vector := by
  exact attested.vectorsPass vector member

theorem attestation_binds_verifier_semantics
    {expectedArtifact : VerifierArtifactIdentity}
    {data : VerifierAttestationData}
    {digest : DomainDigest}
    {decoder : PacketDecoder}
    {verifier : RuntimePathVerifier}
    {leafHash : LeafHasher}
    {branch : BranchHasher}
    (attested : VerifierAttestation expectedArtifact data digest decoder
      verifier leafHash branch) :
    RuntimeVerifierConforms verifier leafHash branch := by
  exact attested.verifierConforms

theorem axle_receipt_binds_lean_module
    {moduleDigest auditDigest artifactDigest vectorDigest : Nat}
    {receipt : AxleVerifierReceiptData}
    (accepted : AxleVerifierReceiptAccepted moduleDigest auditDigest
      artifactDigest vectorDigest receipt) :
    receipt.leanModuleDigest = moduleDigest := by
  exact accepted.1

theorem axle_receipt_binds_lean_audit
    {moduleDigest auditDigest artifactDigest vectorDigest : Nat}
    {receipt : AxleVerifierReceiptData}
    (accepted : AxleVerifierReceiptAccepted moduleDigest auditDigest
      artifactDigest vectorDigest receipt) :
    receipt.leanAuditDigest = auditDigest := by
  exact accepted.2.1

theorem axle_receipt_binds_verifier_artifact
    {moduleDigest auditDigest artifactDigest vectorDigest : Nat}
    {receipt : AxleVerifierReceiptData}
    (accepted : AxleVerifierReceiptAccepted moduleDigest auditDigest
      artifactDigest vectorDigest receipt) :
    receipt.verifierArtifactDigest = artifactDigest := by
  exact accepted.2.2.1

theorem axle_receipt_binds_vector_suite
    {moduleDigest auditDigest artifactDigest vectorDigest : Nat}
    {receipt : AxleVerifierReceiptData}
    (accepted : AxleVerifierReceiptAccepted moduleDigest auditDigest
      artifactDigest vectorDigest receipt) :
    receipt.vectorSuiteDigest = vectorDigest := by
  exact accepted.2.2.2

theorem unbound_axle_receipt_constructible
    (noteDigest : Nat) :
    ∃ receipt : UnboundAxleReceipt,
      receipt.claimedPass = true ∧ receipt.noteDigest = noteDigest := by
  exact ⟨⟨true, noteDigest⟩, rfl, rfl⟩

theorem mismatched_axle_receipt_is_rejected
    (moduleDigest auditDigest artifactDigest vectorDigest : Nat) :
    ¬ AxleVerifierReceiptAccepted moduleDigest auditDigest
      artifactDigest vectorDigest {
        leanModuleDigest := moduleDigest + 1
        leanAuditDigest := auditDigest
        verifierArtifactDigest := artifactDigest
        vectorSuiteDigest := vectorDigest
  } := by
  intro accepted
  exact (Nat.ne_of_gt (Nat.lt_succ_self moduleDigest)) accepted.1

end ASPProof.AgentSessionHostRegistryVerifierAttestation
