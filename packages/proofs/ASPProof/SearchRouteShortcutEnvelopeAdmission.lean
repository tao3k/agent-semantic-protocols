import ASPProof.SearchRouteShortcutCommitmentReplay

namespace ASPProof.SearchRouteShortcutEnvelopeAdmission

open SearchRouteFiniteMultiobjectiveParetoMaskWitness
open SearchRouteDominanceTransitiveShortcut
open SearchRouteDerivedCapacityRank
open SearchRouteShortcutCommitmentReplay

structure RawShortcutEnvelope where
  schemaVersion : Nat
  hashAlgorithmId : Nat
  canonicalPayloadDigest : Nat
  commitment : ShortcutCommitment
  claimedDirectDominance : Bool

structure ShortcutAdmissionPolicy where
  expectedSchemaVersion : Nat
  expectedHashAlgorithmId : Nat
  recomputedCanonicalPayloadDigest : Nat
  expectedCommitment : ShortcutCommitment

def admissionGate
    (policy : ShortcutAdmissionPolicy)
    (raw : RawShortcutEnvelope) : Bool :=
  natEq policy.expectedSchemaVersion raw.schemaVersion &&
    natEq policy.expectedHashAlgorithmId raw.hashAlgorithmId &&
    natEq
      policy.recomputedCanonicalPayloadDigest
      raw.canonicalPayloadDigest &&
    shortcutCommitmentMatches
      policy.expectedCommitment raw.commitment &&
    raw.claimedDirectDominance

structure AdmissionGateFacts
    (policy : ShortcutAdmissionPolicy)
    (raw : RawShortcutEnvelope) : Prop where
  schemaVersionMatches :
    policy.expectedSchemaVersion = raw.schemaVersion
  hashAlgorithmMatches :
    policy.expectedHashAlgorithmId = raw.hashAlgorithmId
  canonicalPayloadDigestMatches :
    policy.recomputedCanonicalPayloadDigest =
      raw.canonicalPayloadDigest
  commitmentMatches :
    policy.expectedCommitment = raw.commitment
  directDominanceClaimed :
    raw.claimedDirectDominance = true

theorem admission_gate_true_implies_facts
    (policy : ShortcutAdmissionPolicy)
    (raw : RawShortcutEnvelope)
    (passed : admissionGate policy raw = true) :
    AdmissionGateFacts policy raw := by
  unfold admissionGate at passed
  have parts5 := bool_and_true_parts _ _ passed
  have parts4 := bool_and_true_parts _ _ parts5.1
  have parts3 := bool_and_true_parts _ _ parts4.1
  have parts2 := bool_and_true_parts _ _ parts3.1
  exact {
    schemaVersionMatches :=
      natEq_true_implies_eq _ _ parts2.1
    hashAlgorithmMatches :=
      natEq_true_implies_eq _ _ parts2.2
    canonicalPayloadDigestMatches :=
      natEq_true_implies_eq _ _ parts3.2
    commitmentMatches :=
      commitment_match_true_implies_equal
        policy.expectedCommitment raw.commitment parts4.2
    directDominanceClaimed := parts5.2
  }

theorem admission_facts_imply_gate_true
    (policy : ShortcutAdmissionPolicy)
    (raw : RawShortcutEnvelope)
    (facts : AdmissionGateFacts policy raw) :
    admissionGate policy raw = true := by
  unfold admissionGate
  apply bool_and_true_of_parts
  · apply bool_and_true_of_parts
    · apply bool_and_true_of_parts
      · apply bool_and_true_of_parts
        · rw [facts.schemaVersionMatches]
          exact natEq_is_reflexive _
        · rw [facts.hashAlgorithmMatches]
          exact natEq_is_reflexive _
      · rw [facts.canonicalPayloadDigestMatches]
        exact natEq_is_reflexive _
    · rw [facts.commitmentMatches]
      exact equal_commitments_match _
  · exact facts.directDominanceClaimed

structure AdmissionWitness
    (policy : ShortcutAdmissionPolicy)
    (raw : RawShortcutEnvelope)
    (start endpoint : Candidate) : Type where
  gateFacts : AdmissionGateFacts policy raw
  endpointDominatesStart :
    StrictDominates endpoint start = true

structure VerifiedCachedShortcut
    (policy : ShortcutAdmissionPolicy)
    (start endpoint : Candidate) where
  raw : RawShortcutEnvelope
  admission : AdmissionWitness policy raw start endpoint

def VerifiedCachedShortcut.toCachedShortcut
    {policy : ShortcutAdmissionPolicy}
    {start endpoint : Candidate}
    (verified : VerifiedCachedShortcut policy start endpoint) :
    CachedShortcut start endpoint := {
  commitment := verified.raw.commitment
  endpointDominatesStart :=
    verified.admission.endpointDominatesStart
}

def verifyRawShortcut
    (policy : ShortcutAdmissionPolicy)
    (raw : RawShortcutEnvelope)
    (start endpoint : Candidate)
    (witness : AdmissionWitness policy raw start endpoint) :
    VerifiedCachedShortcut policy start endpoint := {
  raw := raw
  admission := witness
}

inductive AdmissionOutcome
    (policy : ShortcutAdmissionPolicy)
    (start endpoint : Candidate) where
  | verified :
      VerifiedCachedShortcut policy start endpoint →
        AdmissionOutcome policy start endpoint
  | explicitChainFallback :
      AdmissionOutcome policy start endpoint

def admitRawShortcut
    (policy : ShortcutAdmissionPolicy)
    (raw : RawShortcutEnvelope)
    (start endpoint : Candidate) :
    Option (AdmissionWitness policy raw start endpoint) →
      AdmissionOutcome policy start endpoint
  | none => AdmissionOutcome.explicitChainFallback
  | some witness =>
      AdmissionOutcome.verified
        (verifyRawShortcut policy raw start endpoint witness)

theorem raw_without_witness_falls_back
    (policy : ShortcutAdmissionPolicy)
    (raw : RawShortcutEnvelope)
    (start endpoint : Candidate) :
    admitRawShortcut policy raw start endpoint none =
      AdmissionOutcome.explicitChainFallback := by
  rfl

theorem witnessed_raw_becomes_verified
    (policy : ShortcutAdmissionPolicy)
    (raw : RawShortcutEnvelope)
    (start endpoint : Candidate)
    (witness : AdmissionWitness policy raw start endpoint) :
    admitRawShortcut policy raw start endpoint (some witness) =
      AdmissionOutcome.verified
        (verifyRawShortcut policy raw start endpoint witness) := by
  rfl

def replayVerifiedAgainst
    (expected : ShortcutCommitment)
    {policy : ShortcutAdmissionPolicy}
    {start endpoint : Candidate}
    (verified : VerifiedCachedShortcut policy start endpoint) : Bool :=
  replayAccepts expected verified.toCachedShortcut

theorem admitted_shortcut_is_accepted_under_policy
    (policy : ShortcutAdmissionPolicy)
    (start endpoint : Candidate)
    (verified : VerifiedCachedShortcut policy start endpoint) :
    replayVerifiedAgainst policy.expectedCommitment verified = true := by
  unfold replayVerifiedAgainst
  unfold replayAccepts
  have commitmentEqual :
      policy.expectedCommitment =
        verified.toCachedShortcut.commitment := by
    exact verified.admission.gateFacts.commitmentMatches
  rw [commitmentEqual]
  exact equal_commitments_match _

theorem accepted_verified_replay_preserves_removal_soundness
    (expected : ShortcutCommitment)
    (policy : ShortcutAdmissionPolicy)
    (start endpoint : Candidate)
    (verified : VerifiedCachedShortcut policy start endpoint)
    (_accepted : replayVerifiedAgainst expected verified = true) :
    StrictDominates endpoint start = true :=
  verified.toCachedShortcut.endpointDominatesStart

theorem changed_context_rejects_verified_shortcut
    (expected : ShortcutCommitment)
    (policy : ShortcutAdmissionPolicy)
    (start endpoint : Candidate)
    (verified : VerifiedCachedShortcut policy start endpoint)
    (different :
      expected ≠ verified.toCachedShortcut.commitment) :
    replayVerifiedAgainst expected verified = false :=
  mismatched_replay_is_rejected
    expected start endpoint verified.toCachedShortcut different

theorem schema_mismatch_blocks_admission_witness
    (policy : ShortcutAdmissionPolicy)
    (raw : RawShortcutEnvelope)
    (start endpoint : Candidate)
    (different :
      policy.expectedSchemaVersion ≠ raw.schemaVersion) :
    AdmissionWitness policy raw start endpoint → False := by
  intro witness
  exact different witness.gateFacts.schemaVersionMatches

theorem hash_algorithm_mismatch_blocks_admission_witness
    (policy : ShortcutAdmissionPolicy)
    (raw : RawShortcutEnvelope)
    (start endpoint : Candidate)
    (different :
      policy.expectedHashAlgorithmId ≠ raw.hashAlgorithmId) :
    AdmissionWitness policy raw start endpoint → False := by
  intro witness
  exact different witness.gateFacts.hashAlgorithmMatches

theorem canonical_digest_mismatch_blocks_admission_witness
    (policy : ShortcutAdmissionPolicy)
    (raw : RawShortcutEnvelope)
    (start endpoint : Candidate)
    (different :
      policy.recomputedCanonicalPayloadDigest ≠
        raw.canonicalPayloadDigest) :
    AdmissionWitness policy raw start endpoint → False := by
  intro witness
  exact different
    witness.gateFacts.canonicalPayloadDigestMatches

theorem commitment_mismatch_blocks_admission_witness
    (policy : ShortcutAdmissionPolicy)
    (raw : RawShortcutEnvelope)
    (start endpoint : Candidate)
    (different : policy.expectedCommitment ≠ raw.commitment) :
    AdmissionWitness policy raw start endpoint → False := by
  intro witness
  exact different witness.gateFacts.commitmentMatches

theorem false_dominance_claim_blocks_admission_witness
    (policy : ShortcutAdmissionPolicy)
    (raw : RawShortcutEnvelope)
    (start endpoint : Candidate)
    (claimedFalse : raw.claimedDirectDominance = false) :
    AdmissionWitness policy raw start endpoint → False := by
  intro witness
  have impossible : false = true :=
    claimedFalse.symm.trans
      witness.gateFacts.directDominanceClaimed
  cases impossible

def rawAdmissionProjectionTokens : Nat :=
  5

def verifiedReplayProjectionTokens : Nat :=
  6

def explicitChainFallbackTokens (chainLength : Nat) : Nat :=
  6 + chainLength

theorem verified_replay_projection_is_constant_shape :
    verifiedReplayProjectionTokens = 6 := by
  rfl

theorem verified_replay_beats_nonempty_fallback_projection
    (chainLength : Nat) :
    verifiedReplayProjectionTokens <
      explicitChainFallbackTokens (Nat.succ chainLength) := by
  unfold verifiedReplayProjectionTokens
  unfold explicitChainFallbackTokens
  exact Nat.add_lt_add_left
    (Nat.zero_lt_succ chainLength) 6

end ASPProof.SearchRouteShortcutEnvelopeAdmission
