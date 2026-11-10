-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchLoopCache
import ASPProof.SearchLoopCacheIdentity

namespace SearchLoopCacheRefinement

open SearchLoopCache
open SearchLoopCacheIdentity

def projectSemanticKey
    (key : SearchLoopCache.SemanticCacheKey) :
    SearchLoopCacheIdentity.SemanticKey :=
  { obligation := key.obligation
    workspace := key.workspace
    witnessSet := key.witness
    domain :=
      { sourceSnapshot := key.witness
        provider := key.provider
        schema := key.schema
        policy := key.policy }
    providerArtifact := key.providerArtifact
    selector := key.selector
    rfc := key.rfc
    projection := key.projection
    budgetClass := key.budgetClass }

theorem projectSemanticKey_injective :
    Function.Injective projectSemanticKey := by
  intro left right projectedEqual
  cases left
  cases right
  simp [projectSemanticKey] at projectedEqual
  simp_all

def eraseReason :
    SearchLoopCache.ReuseDecision →
      SearchLoopCacheIdentity.CacheDecision
  | SearchLoopCache.ReuseDecision.hit =>
      SearchLoopCacheIdentity.CacheDecision.reuse
  | SearchLoopCache.ReuseDecision.stale _ =>
      SearchLoopCacheIdentity.CacheDecision.invalidate

theorem hit_iff_projected_reuse
    (cached current : SearchLoopCache.SemanticCacheKey) :
    SearchLoopCache.validateSemantic cached current =
        SearchLoopCache.ReuseDecision.hit ↔
      SearchLoopCacheIdentity.decideSemanticReuse
          (projectSemanticKey cached)
          (projectSemanticKey current) =
        SearchLoopCacheIdentity.CacheDecision.reuse := by
  rw
    [ SearchLoopCache.validateSemantic_hit_iff_equal
    , SearchLoopCacheIdentity.semantic_reuse_iff_exact_key
    ]
  constructor
  · intro equalKeys
    cases equalKeys
    rfl
  · intro projectedEqual
    exact projectSemanticKey_injective projectedEqual

theorem erase_validateSemantic_eq_projected_decision
    (cached current : SearchLoopCache.SemanticCacheKey) :
    eraseReason (SearchLoopCache.validateSemantic cached current) =
      SearchLoopCacheIdentity.decideSemanticReuse
        (projectSemanticKey cached)
        (projectSemanticKey current) := by
  by_cases equalKeys : cached = current
  · cases equalKeys
    rw [SearchLoopCache.validateSemantic_self]
    simpa [eraseReason] using
      (SearchLoopCacheIdentity.stable_semantic_key_reuses
        (projectSemanticKey cached)).symm
  · have projectedDrift :
        projectSemanticKey cached ≠ projectSemanticKey current := by
      intro projectedEqual
      exact equalKeys (projectSemanticKey_injective projectedEqual)
    have notHit :
        SearchLoopCache.validateSemantic cached current ≠
          SearchLoopCache.ReuseDecision.hit := by
      intro hit
      exact equalKeys
        (SearchLoopCache.validateSemantic_hit_implies_equal
          cached current hit)
    cases decisionEq :
        SearchLoopCache.validateSemantic cached current with
    | hit =>
        exact False.elim (notHit decisionEq)
    | stale reason =>
        simp only [eraseReason]
        exact
          (SearchLoopCacheIdentity.semantic_key_drift_invalidates
            (projectSemanticKey cached)
            (projectSemanticKey current)
            projectedDrift).symm

def providerAndPolicyDrift :
    SearchLoopCache.SemanticCacheKey :=
  { SearchLoopCache.baseSemanticKey with
    provider := 200
    policy := 900 }

def providerArtifactAndSelectorDrift :
    SearchLoopCache.SemanticCacheKey :=
  { SearchLoopCache.baseSemanticKey with
    providerArtifact := 300
    selector := 700 }

def selectorAndSchemaDrift :
    SearchLoopCache.SemanticCacheKey :=
  { SearchLoopCache.baseSemanticKey with
    selector := 700
    schema := 800 }

theorem provider_precedes_policy_reason :
    SearchLoopCache.validateSemantic
        SearchLoopCache.baseSemanticKey
        providerAndPolicyDrift =
      SearchLoopCache.ReuseDecision.stale
        SearchLoopCache.InvalidationReason.provider := by
  decide

theorem provider_artifact_precedes_selector_reason :
    SearchLoopCache.validateSemantic
        SearchLoopCache.baseSemanticKey
        providerArtifactAndSelectorDrift =
      SearchLoopCache.ReuseDecision.stale
        SearchLoopCache.InvalidationReason.providerArtifact := by
  decide

theorem selector_precedes_schema_reason :
    SearchLoopCache.validateSemantic
        SearchLoopCache.baseSemanticKey
        selectorAndSchemaDrift =
      SearchLoopCache.ReuseDecision.stale
        SearchLoopCache.InvalidationReason.selector := by
  decide

theorem reason_precedence_does_not_change_authority :
    eraseReason
        (SearchLoopCache.validateSemantic
          SearchLoopCache.baseSemanticKey
          providerAndPolicyDrift) =
        SearchLoopCacheIdentity.CacheDecision.invalidate ∧
      eraseReason
          (SearchLoopCache.validateSemantic
            SearchLoopCache.baseSemanticKey
            providerArtifactAndSelectorDrift) =
          SearchLoopCacheIdentity.CacheDecision.invalidate ∧
      eraseReason
          (SearchLoopCache.validateSemantic
            SearchLoopCache.baseSemanticKey
            selectorAndSchemaDrift) =
          SearchLoopCacheIdentity.CacheDecision.invalidate := by
  decide

end SearchLoopCacheRefinement
