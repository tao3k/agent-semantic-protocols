import ASPProof.SearchRouteCanonicalPayloadDigestReplay

namespace ASPProof.SemanticProjectionRuntimeWrapping

structure ProviderPayload where
  graphDigest : Nat
  claimedGeneration : Nat
  claimedRoot : Nat

structure RuntimeAuthority where
  generation : Nat
  root : Nat
  provider : Nat

structure SemanticEnvelope where
  authority : RuntimeAuthority
  payloadDigest : Nat

def wrap (authority : RuntimeAuthority) (payload : ProviderPayload) : SemanticEnvelope :=
  { authority := authority, payloadDigest := payload.graphDigest }

theorem provider_payload_cannot_supply_envelope_authority
    (authority : RuntimeAuthority) (payload : ProviderPayload) :
    (wrap authority payload).authority = authority := by
  rfl

theorem verified_binding_preserves_runtime_authority
    (left right : RuntimeAuthority) (payload : ProviderPayload)
    (sameGeneration : left.generation = right.generation)
    (sameRoot : left.root = right.root)
    (sameProvider : left.provider = right.provider) :
    (wrap left payload).authority = (wrap right payload).authority := by
  cases left
  cases right
  simp_all [wrap]

def admitted (authority : RuntimeAuthority) (payload : ProviderPayload)
    (expectedDigest : Nat) : Prop :=
  (wrap authority payload).payloadDigest = expectedDigest

theorem payload_digest_mismatch_rejected
    (authority : RuntimeAuthority) (payload : ProviderPayload)
    (expectedDigest : Nat) (mismatch : payload.graphDigest ≠ expectedDigest) :
    ¬ admitted authority payload expectedDigest := by
  intro accepted
  exact mismatch accepted

end ASPProof.SemanticProjectionRuntimeWrapping
