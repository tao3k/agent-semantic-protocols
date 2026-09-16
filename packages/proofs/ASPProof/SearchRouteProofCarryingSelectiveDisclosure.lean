-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteCertifiedTransitionChainCompression

namespace ASPProof.SearchRouteProofCarryingSelectiveDisclosure

open ASPProof.SearchRouteCertifiedRegistryEpochTransition
open ASPProof.SearchRouteCertifiedTransitionChainCompression

structure CacheReuse where
  searchCacheHit : Bool
  proofCacheHit : Bool
  modelPrefixCacheHit : Bool
deriving DecidableEq, Repr

structure DisclosureRequest where
  summary : ChainSummary
  policyDigest : Digest
  queryDigest : Digest
  requiredEdges : List Nat
deriving DecidableEq, Repr

structure DisclosureReceipt where
  summary : ChainSummary
  policyDigest : Digest
  queryDigest : Digest
  disclosedEdges : List Nat
  cacheReuse : CacheReuse
deriving DecidableEq, Repr

def IdentityBound
    (request : DisclosureRequest)
    (receipt : DisclosureReceipt) : Prop :=
  receipt.summary = request.summary ∧
    receipt.policyDigest = request.policyDigest ∧
    receipt.queryDigest = request.queryDigest

def CanonicalRequest (request : DisclosureRequest) : Prop :=
  request.requiredEdges.Pairwise (fun left right => left < right)

def CoversRequired
    (request : DisclosureRequest)
    (receipt : DisclosureReceipt) : Prop :=
  request.requiredEdges.Sublist receipt.disclosedEdges

def AllBelow (bound : Nat) : List Nat → Prop
  | [] => True
  | edge :: rest => edge < bound ∧ AllBelow bound rest

def ValidDisclosure
    (request : DisclosureRequest)
    (receipt : DisclosureReceipt) : Prop :=
  IdentityBound request receipt ∧
    CanonicalRequest request ∧
    CoversRequired request receipt ∧
    AllBelow request.summary.edgeCount receipt.disclosedEdges

def TokenMinimalDisclosure
    (request : DisclosureRequest)
    (receipt : DisclosureReceipt) : Prop :=
  ValidDisclosure request receipt ∧
    ∀ candidate,
      ValidDisclosure request candidate →
      receipt.disclosedEdges.length ≤ candidate.disclosedEdges.length

def AuthorizedDecision
    (chainVerified : Prop)
    (request : DisclosureRequest)
    (receipt : DisclosureReceipt) : Prop :=
  chainVerified ∧ ValidDisclosure request receipt

structure SearchCacheKey where
  queryDigest : Digest
  providerDigest : Digest
  registrySnapshotDigest : Digest
deriving DecidableEq, Repr

structure ProofCacheKey where
  chainDigest : Digest
  policyDigest : Digest
  auditDigest : Digest
deriving DecidableEq, Repr

structure ModelPrefixCacheKey where
  modelDigest : Digest
  serializedPrefixDigest : Digest
deriving DecidableEq, Repr

structure WorkCost where
  providerCalls : Nat
  proofChecks : Nat
  disclosedTokens : Nat
  uncachedModelTokens : Nat
  llmRounds : Nat
deriving DecidableEq, Repr

def applyCacheReuse (baseline : WorkCost) (reuse : CacheReuse) : WorkCost :=
  {
    providerCalls :=
      if reuse.searchCacheHit then 0 else baseline.providerCalls
    proofChecks :=
      if reuse.proofCacheHit then 0 else baseline.proofChecks
    disclosedTokens := baseline.disclosedTokens
    uncachedModelTokens :=
      if reuse.modelPrefixCacheHit then 0 else baseline.uncachedModelTokens
    llmRounds := baseline.llmRounds
  }

def searchOnlyHit : CacheReuse :=
  { searchCacheHit := true, proofCacheHit := false,
    modelPrefixCacheHit := false }

def proofOnlyHit : CacheReuse :=
  { searchCacheHit := false, proofCacheHit := true,
    modelPrefixCacheHit := false }

def modelPrefixOnlyHit : CacheReuse :=
  { searchCacheHit := false, proofCacheHit := false,
    modelPrefixCacheHit := true }

def allCacheMisses : CacheReuse :=
  { searchCacheHit := false, proofCacheHit := false,
    modelPrefixCacheHit := false }

theorem valid_disclosure_binds_identity
    {request : DisclosureRequest}
    {receipt : DisclosureReceipt}
    (valid : ValidDisclosure request receipt) :
    IdentityBound request receipt :=
  valid.1

theorem valid_disclosure_covers_required_edges
    {request : DisclosureRequest}
    {receipt : DisclosureReceipt}
    (valid : ValidDisclosure request receipt) :
    CoversRequired request receipt :=
  valid.2.2.1

theorem all_below_member
    {bound edge : Nat}
    {edges : List Nat}
    (bounded : AllBelow bound edges)
    (member : edge ∈ edges) :
    edge < bound := by
  induction edges with
  | nil =>
      simp at member
  | cons head tail inductionHypothesis =>
      simp only [List.mem_cons] at member
      cases member with
      | inl equal =>
          subst edge
          exact bounded.1
      | inr tailMember =>
          exact inductionHypothesis bounded.2 tailMember

theorem valid_disclosure_rejects_out_of_range_edges
    {request : DisclosureRequest}
    {receipt : DisclosureReceipt}
    (valid : ValidDisclosure request receipt)
    {edge : Nat}
    (disclosed : edge ∈ receipt.disclosedEdges) :
    edge < request.summary.edgeCount :=
  all_below_member valid.2.2.2 disclosed

theorem exact_required_edges_are_token_minimal
    {request : DisclosureRequest}
    {receipt : DisclosureReceipt}
    (identity : IdentityBound request receipt)
    (canonical : CanonicalRequest request)
    (bounded :
      AllBelow request.summary.edgeCount request.requiredEdges)
    (exact : receipt.disclosedEdges = request.requiredEdges) :
    TokenMinimalDisclosure request receipt := by
  constructor
  · refine ⟨identity, canonical, ?_, ?_⟩
    · show request.requiredEdges.Sublist receipt.disclosedEdges
      rw [exact]
      exact List.Sublist.refl request.requiredEdges
    · rw [exact]
      exact bounded
  · intro candidate candidateValid
    rw [exact]
    exact candidateValid.2.2.1.length_le

theorem authorized_decision_requires_verified_chain
    {chainVerified : Prop}
    {request : DisclosureRequest}
    {receipt : DisclosureReceipt}
    (authorized :
      AuthorizedDecision chainVerified request receipt) :
    chainVerified :=
  authorized.1

theorem authorized_decision_requires_valid_disclosure
    {chainVerified : Prop}
    {request : DisclosureRequest}
    {receipt : DisclosureReceipt}
    (authorized :
      AuthorizedDecision chainVerified request receipt) :
    ValidDisclosure request receipt :=
  authorized.2

theorem search_cache_identity_does_not_determine_proof_identity :
    let searchLeft : SearchCacheKey :=
      { queryDigest := 1, providerDigest := 2,
        registrySnapshotDigest := 3 }
    let searchRight : SearchCacheKey := searchLeft
    let proofLeft : ProofCacheKey :=
      { chainDigest := 4, policyDigest := 5, auditDigest := 6 }
    let proofRight : ProofCacheKey :=
      { chainDigest := 7, policyDigest := 5, auditDigest := 6 }
    searchLeft = searchRight ∧ proofLeft ≠ proofRight := by
  decide

theorem proof_cache_identity_does_not_determine_model_prefix_identity :
    let proofLeft : ProofCacheKey :=
      { chainDigest := 4, policyDigest := 5, auditDigest := 6 }
    let proofRight : ProofCacheKey := proofLeft
    let modelLeft : ModelPrefixCacheKey :=
      { modelDigest := 8, serializedPrefixDigest := 9 }
    let modelRight : ModelPrefixCacheKey :=
      { modelDigest := 8, serializedPrefixDigest := 10 }
    proofLeft = proofRight ∧ modelLeft ≠ modelRight := by
  decide

theorem model_prefix_identity_does_not_determine_search_identity :
    let modelLeft : ModelPrefixCacheKey :=
      { modelDigest := 8, serializedPrefixDigest := 9 }
    let modelRight : ModelPrefixCacheKey := modelLeft
    let searchLeft : SearchCacheKey :=
      { queryDigest := 1, providerDigest := 2,
        registrySnapshotDigest := 3 }
    let searchRight : SearchCacheKey :=
      { queryDigest := 11, providerDigest := 2,
        registrySnapshotDigest := 3 }
    modelLeft = modelRight ∧ searchLeft ≠ searchRight := by
  decide

theorem search_cache_hit_reduces_only_provider_calls
    (baseline : WorkCost) :
    applyCacheReuse baseline searchOnlyHit = {
      providerCalls := 0
      proofChecks := baseline.proofChecks
      disclosedTokens := baseline.disclosedTokens
      uncachedModelTokens := baseline.uncachedModelTokens
      llmRounds := baseline.llmRounds
    } :=
  rfl

theorem proof_cache_hit_reduces_only_proof_checks
    (baseline : WorkCost) :
    applyCacheReuse baseline proofOnlyHit = {
      providerCalls := baseline.providerCalls
      proofChecks := 0
      disclosedTokens := baseline.disclosedTokens
      uncachedModelTokens := baseline.uncachedModelTokens
      llmRounds := baseline.llmRounds
    } :=
  rfl

theorem model_prefix_hit_reduces_only_uncached_model_tokens
    (baseline : WorkCost) :
    applyCacheReuse baseline modelPrefixOnlyHit = {
      providerCalls := baseline.providerCalls
      proofChecks := baseline.proofChecks
      disclosedTokens := baseline.disclosedTokens
      uncachedModelTokens := 0
      llmRounds := baseline.llmRounds
    } :=
  rfl

theorem all_cache_misses_preserve_baseline
    (baseline : WorkCost) :
    applyCacheReuse baseline allCacheMisses = baseline :=
  rfl

end ASPProof.SearchRouteProofCarryingSelectiveDisclosure
