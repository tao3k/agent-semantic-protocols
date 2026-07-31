import ASPProof.SearchRouteAdmissionRetryPublicationKeyEncoding

namespace ASPProof.SearchRouteAdmissionRetryPublicationDigest

universe u v

inductive DigestDomain where
  | retryPublicationKey
  | evidencePayload
  | modelCacheIdentity
deriving DecidableEq, Repr

inductive DigestAlgorithm where
  | blake3V1
  | sha256V1
deriving DecidableEq, Repr

structure DigestPreimage (Payload : Type u) where
  domain : DigestDomain
  payload : Payload
deriving DecidableEq, Repr

structure DigestReference (Digest : Type v) where
  algorithm : DigestAlgorithm
  domain : DigestDomain
  value : Digest
deriving DecidableEq, Repr

/--
Lean treats collision freedom as an explicit property of the selected digest
function. A concrete cryptographic algorithm must justify this premise outside
the purely logical model.
-/
def CollisionFree
    {Payload : Type u}
    {Digest : Type v}
    (digest : DigestPreimage Payload → Digest) : Prop :=
  Function.Injective digest

theorem collision_free_digest_equality_preserves_domain_and_payload
    {Payload : Type u}
    {Digest : Type v}
    {digest : DigestPreimage Payload → Digest}
    (collisionFree : CollisionFree digest)
    {left right : DigestPreimage Payload}
    (equalDigest : digest left = digest right) :
    left.domain = right.domain
      ∧ left.payload = right.payload := by
  have equalPreimage := collisionFree equalDigest
  cases equalPreimage
  exact ⟨rfl, rfl⟩

def constantDigest
    {Payload : Type u}
    (_preimage : DigestPreimage Payload) : Nat :=
  0

def retryKeyPreimage : DigestPreimage Bool :=
  { domain := DigestDomain.retryPublicationKey
    payload := true }

def evidencePreimage : DigestPreimage Bool :=
  { domain := DigestDomain.evidencePayload
    payload := true }

theorem domain_tagged_preimages_do_not_make_arbitrary_digest_collision_free :
    retryKeyPreimage ≠ evidencePreimage
      ∧ constantDigest retryKeyPreimage =
        constantDigest evidencePreimage
      ∧ ¬ CollisionFree (constantDigest : DigestPreimage Bool → Nat) := by
  refine ⟨by decide, rfl, ?_⟩
  intro collisionFree
  have equalPreimage :
      retryKeyPreimage = evidencePreimage :=
    collisionFree rfl
  exact (by decide : retryKeyPreimage ≠ evidencePreimage) equalPreimage

def omitDomainDigest (preimage : DigestPreimage Bool) : Bool :=
  preimage.payload

theorem omitting_domain_tag_aliases_equal_payloads_across_protocols :
    retryKeyPreimage ≠ evidencePreimage
      ∧ omitDomainDigest retryKeyPreimage =
        omitDomainDigest evidencePreimage := by
  decide

def blakeReference : DigestReference Nat :=
  { algorithm := DigestAlgorithm.blake3V1
    domain := DigestDomain.retryPublicationKey
    value := 7 }

def shaReference : DigestReference Nat :=
  { algorithm := DigestAlgorithm.sha256V1
    domain := DigestDomain.retryPublicationKey
    value := 7 }

def omitAlgorithm
    {Digest : Type v}
    (reference : DigestReference Digest) :
    DigestDomain × Digest :=
  (reference.domain, reference.value)

theorem omitting_algorithm_identity_aliases_distinct_digest_references :
    blakeReference ≠ shaReference
      ∧ omitAlgorithm blakeReference =
        omitAlgorithm shaReference := by
  decide

def truncatedDigest (identity : Nat) : Nat :=
  identity % 10

theorem digest_truncation_is_not_collision_free :
    (1 : Nat) ≠ 11
      ∧ truncatedDigest 1 = truncatedDigest 11
      ∧ ¬ Function.Injective truncatedDigest := by
  refine ⟨by decide, by decide, ?_⟩
  intro injective
  exact (by decide : (1 : Nat) ≠ 11) (injective (by decide))

end ASPProof.SearchRouteAdmissionRetryPublicationDigest
