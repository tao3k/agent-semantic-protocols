import ASPProof.SearchRouteInspectCommittedUniverse

namespace SearchRouteInspectFiniteCommitment

open SearchRouteDAG
open SearchRouteInspectCommittedUniverse

structure IdealContentIdentity where
  encodeCandidate : GraphCandidate → Nat
  encodeInjective : Function.Injective encodeCandidate
  digest : List Nat → Nat
  digestInjective : Function.Injective digest
  universeDomainTag : Nat
  frontierDomainTag : Nat
  domainTagsDistinct : universeDomainTag ≠ frontierDomainTag

structure CanonicalManifest (identity : IdealContentIdentity) where
  codes : List Nat
  canonical : codes.Pairwise (fun left right => left < right)

def universeRoot
    (identity : IdealContentIdentity)
    (manifest : CanonicalManifest identity) :
    Nat :=
  identity.digest (identity.universeDomainTag :: manifest.codes)

def frontierRoot
    (identity : IdealContentIdentity)
    (manifest : CanonicalManifest identity) :
    Nat :=
  identity.digest (identity.frontierDomainTag :: manifest.codes)

def finiteValidRoot
    (identity : IdealContentIdentity)
    (root : Nat) :
    Prop :=
  ∃ manifest : CanonicalManifest identity,
    universeRoot identity manifest = root

def finiteContains
    (identity : IdealContentIdentity)
    (root : Nat)
    (candidate : GraphCandidate) :
    Prop :=
  ∃ manifest : CanonicalManifest identity,
    universeRoot identity manifest = root ∧
      identity.encodeCandidate candidate ∈ manifest.codes

def finiteVerifyMembership
    (identity : IdealContentIdentity)
    (root : Nat)
    (candidate : GraphCandidate)
    (proof : CanonicalManifest identity) :
    Bool :=
  decide (
    universeRoot identity proof = root ∧
      identity.encodeCandidate candidate ∈ proof.codes)

def finiteVerifyExclusion
    (identity : IdealContentIdentity)
    (root : Nat)
    (candidate : GraphCandidate)
    (proof : CanonicalManifest identity) :
    Bool :=
  decide (
    universeRoot identity proof = root ∧
      identity.encodeCandidate candidate ∉ proof.codes)

theorem universe_root_binding
    {identity : IdealContentIdentity}
    {left right : CanonicalManifest identity}
    (rootsEqual :
      universeRoot identity left = universeRoot identity right) :
    left.codes = right.codes := by
  have payloadsEqual :
      identity.universeDomainTag :: left.codes =
        identity.universeDomainTag :: right.codes :=
    identity.digestInjective rootsEqual
  exact List.cons.inj payloadsEqual |>.2

theorem universe_and_frontier_roots_are_domain_separated
    (identity : IdealContentIdentity)
    (universeManifest frontierManifest : CanonicalManifest identity) :
    universeRoot identity universeManifest ≠
      frontierRoot identity frontierManifest := by
  intro rootsEqual
  have payloadsEqual :
      identity.universeDomainTag :: universeManifest.codes =
        identity.frontierDomainTag :: frontierManifest.codes :=
    identity.digestInjective rootsEqual
  exact identity.domainTagsDistinct (List.cons.inj payloadsEqual).1

theorem finite_membership_sound
    (identity : IdealContentIdentity)
    (root : Nat)
    (candidate : GraphCandidate)
    (proof : CanonicalManifest identity)
    (verified :
      finiteVerifyMembership identity root candidate proof = true) :
    finiteContains identity root candidate := by
  have decoded :
      universeRoot identity proof = root ∧
        identity.encodeCandidate candidate ∈ proof.codes := by
    simpa [finiteVerifyMembership] using verified
  exact ⟨proof, decoded⟩

theorem finite_membership_complete
    (identity : IdealContentIdentity)
    (root : Nat)
    (candidate : GraphCandidate)
    (contained : finiteContains identity root candidate) :
    ∃ proof,
      finiteVerifyMembership identity root candidate proof = true := by
  rcases contained with ⟨manifest, rootBound, candidateMember⟩
  exact ⟨
    manifest,
    by
      simp [
        finiteVerifyMembership,
        rootBound,
        candidateMember
      ]
  ⟩

theorem finite_contained_root_is_valid
    (identity : IdealContentIdentity)
    (root : Nat)
    (candidate : GraphCandidate)
    (contained : finiteContains identity root candidate) :
    finiteValidRoot identity root := by
  rcases contained with ⟨manifest, rootBound, _⟩
  exact ⟨manifest, rootBound⟩

theorem finite_exclusion_sound
    (identity : IdealContentIdentity)
    (root : Nat)
    (candidate : GraphCandidate)
    (proof : CanonicalManifest identity)
    (verified :
      finiteVerifyExclusion identity root candidate proof = true) :
    ¬finiteContains identity root candidate := by
  have decoded :
      universeRoot identity proof = root ∧
        identity.encodeCandidate candidate ∉ proof.codes := by
    simpa [finiteVerifyExclusion] using verified
  intro contained
  rcases contained with
    ⟨otherManifest, otherRootBound, candidateMember⟩
  have rootsEqual :
      universeRoot identity otherManifest =
        universeRoot identity proof :=
    otherRootBound.trans decoded.1.symm
  have codesEqual :
      otherManifest.codes = proof.codes :=
    universe_root_binding rootsEqual
  exact decoded.2 (codesEqual ▸ candidateMember)

theorem finite_exclusion_root_is_valid
    (identity : IdealContentIdentity)
    (root : Nat)
    (candidate : GraphCandidate)
    (proof : CanonicalManifest identity)
    (verified :
      finiteVerifyExclusion identity root candidate proof = true) :
    finiteValidRoot identity root := by
  have decoded :
      universeRoot identity proof = root ∧
        identity.encodeCandidate candidate ∉ proof.codes := by
    simpa [finiteVerifyExclusion] using verified
  exact ⟨proof, decoded.1⟩

theorem finite_exclusion_complete
    (identity : IdealContentIdentity)
    (root : Nat)
    (candidate : GraphCandidate)
    (rootValid : finiteValidRoot identity root)
    (absent : ¬finiteContains identity root candidate) :
    ∃ proof,
      finiteVerifyExclusion identity root candidate proof = true := by
  rcases rootValid with ⟨manifest, rootBound⟩
  have candidateAbsent :
      identity.encodeCandidate candidate ∉ manifest.codes := by
    intro candidateMember
    exact absent ⟨manifest, rootBound, candidateMember⟩
  exact ⟨
    manifest,
    by
      simp [
        finiteVerifyExclusion,
        rootBound,
        candidateAbsent
      ]
  ⟩

def finiteCandidateCommitmentScheme
    (identity : IdealContentIdentity) :
    CandidateCommitmentScheme where
  MembershipProof := CanonicalManifest identity
  ExclusionProof := CanonicalManifest identity
  validRoot := finiteValidRoot identity
  contains := finiteContains identity
  verifyMembership := finiteVerifyMembership identity
  verifyExclusion := finiteVerifyExclusion identity
  membershipSound := finite_membership_sound identity
  containedRootValid := finite_contained_root_is_valid identity
  membershipComplete := finite_membership_complete identity
  exclusionSound := finite_exclusion_sound identity
  exclusionRootSound := finite_exclusion_root_is_valid identity
  exclusionComplete := finite_exclusion_complete identity

def finiteOpeningCodeCount
    {identity : IdealContentIdentity}
    (proof : CanonicalManifest identity) :
    Nat :=
  proof.codes.length

theorem finite_opening_materializes_manifest
    {identity : IdealContentIdentity}
    (proof : CanonicalManifest identity) :
    finiteOpeningCodeCount proof = proof.codes.length := by
  rfl

theorem finite_verified_membership_and_exclusion_conflict
    {identity : IdealContentIdentity}
    {root : Nat}
    {candidate : GraphCandidate}
    (membership :
      MembershipOpening
        (finiteCandidateCommitmentScheme identity)
        root
        candidate)
    (exclusion :
      ExclusionOpening
        (finiteCandidateCommitmentScheme identity)
        root
        candidate) :
    False :=
  membership_and_exclusion_cannot_both_verify membership exclusion

end SearchRouteInspectFiniteCommitment
