import ASPProof.SearchRouteGraphCostVector

namespace ASPProof.SearchRouteCacheParametricCost

open ASPProof.SearchRouteGraphCostVector

inductive PrefixCacheState where
  | verifiedHit
  | miss
  deriving DecidableEq, Repr

structure SemanticSearchCacheKey where
  evidenceRootDigest : Nat
  queryDigest : Nat
  projectionVersion : Nat
  deriving DecidableEq, Repr

structure ModelPrefixCacheKey where
  modelDigest : Nat
  tokenizerDigest : Nat
  promptPrefixDigest : Nat
  deriving DecidableEq, Repr

structure CostContext where
  semanticKey : SemanticSearchCacheKey
  prefixKey : ModelPrefixCacheKey
  prefixState : PrefixCacheState
  costModelVersion : Nat
  deriving DecidableEq, Repr

structure SemanticEvidenceReceipt where
  semanticKey : SemanticSearchCacheKey
  validated : Bool
  deriving DecidableEq, Repr

structure PromptCostWitness where
  evidenceTokens : Nat
  promptTokens : Nat
  cachedPromptTokens : Nat
  prefixClaimSound : cachedPromptTokens ≤ promptTokens

def effectivePromptTokens
    (state : PrefixCacheState)
    (witness : PromptCostWitness) : Nat :=
  match state with
  | .verifiedHit => witness.promptTokens - witness.cachedPromptTokens
  | .miss => witness.promptTokens

def effectiveTokens
    (state : PrefixCacheState)
    (witness : PromptCostWitness) : Nat :=
  witness.evidenceTokens + effectivePromptTokens state witness

def conservativeTokens (witness : PromptCostWitness) : Nat :=
  witness.evidenceTokens + witness.promptTokens

def vectorFor
    (state : PrefixCacheState)
    (witness : PromptCostWitness)
    (graphHops rounds transitions : Nat) : GraphCostVector :=
  {
    graphHops := graphHops
    uncachedTokens := effectiveTokens state witness
    rounds := rounds
    transitions := transitions
  }

structure CacheBoundCostCertificate where
  claimedContext : CostContext
  vector : GraphCostVector
  witness : PromptCostWitness
  tokenComponentCorrect :
    vector.uncachedTokens =
      effectiveTokens claimedContext.prefixState witness

def CacheBound
    (active : CostContext)
    (certificate : CacheBoundCostCertificate) : Prop :=
  active.semanticKey = certificate.claimedContext.semanticKey ∧
  active.prefixKey = certificate.claimedContext.prefixKey ∧
  active.prefixState = certificate.claimedContext.prefixState ∧
  active.costModelVersion = certificate.claimedContext.costModelVersion

def SemanticReceiptBound
    (active : CostContext)
    (receipt : SemanticEvidenceReceipt) : Prop :=
  active.semanticKey = receipt.semanticKey ∧
  receipt.validated = true

def UsableCostCertificate
    (active : CostContext)
    (receipt : SemanticEvidenceReceipt)
    (certificate : CacheBoundCostCertificate) : Prop :=
  CacheBound active certificate ∧
  SemanticReceiptBound active receipt

theorem cache_bound_refl (certificate : CacheBoundCostCertificate) :
    CacheBound certificate.claimedContext certificate := by
  simp [CacheBound]

theorem semantic_cache_drift_rejects
    (active : CostContext)
    (certificate : CacheBoundCostCertificate)
    (drift :
      active.semanticKey ≠ certificate.claimedContext.semanticKey) :
    ¬ CacheBound active certificate := by
  intro bound
  exact drift bound.1

theorem semantic_receipt_drift_rejects
    (active : CostContext)
    (receipt : SemanticEvidenceReceipt)
    (drift : active.semanticKey ≠ receipt.semanticKey) :
    ¬ SemanticReceiptBound active receipt := by
  intro bound
  exact drift bound.1

theorem semantic_root_drift_rejects
    (active : CostContext)
    (certificate : CacheBoundCostCertificate)
    (drift :
      active.semanticKey.evidenceRootDigest ≠
        certificate.claimedContext.semanticKey.evidenceRootDigest) :
    ¬ CacheBound active certificate := by
  intro bound
  exact drift (congrArg SemanticSearchCacheKey.evidenceRootDigest bound.1)

theorem prefix_cache_drift_rejects
    (active : CostContext)
    (certificate : CacheBoundCostCertificate)
    (drift :
      active.prefixKey ≠ certificate.claimedContext.prefixKey) :
    ¬ CacheBound active certificate := by
  intro bound
  exact drift bound.2.1

theorem model_drift_rejects
    (active : CostContext)
    (certificate : CacheBoundCostCertificate)
    (drift :
      active.prefixKey.modelDigest ≠
        certificate.claimedContext.prefixKey.modelDigest) :
    ¬ CacheBound active certificate := by
  intro bound
  exact drift (congrArg ModelPrefixCacheKey.modelDigest bound.2.1)

theorem prefix_state_drift_rejects
    (active : CostContext)
    (certificate : CacheBoundCostCertificate)
    (drift :
      active.prefixState ≠ certificate.claimedContext.prefixState) :
    ¬ CacheBound active certificate := by
  intro bound
  exact drift bound.2.2.1

theorem cost_model_drift_rejects
    (active : CostContext)
    (certificate : CacheBoundCostCertificate)
    (drift :
      active.costModelVersion ≠
        certificate.claimedContext.costModelVersion) :
    ¬ CacheBound active certificate := by
  intro bound
  exact drift bound.2.2.2

theorem effective_tokens_le_conservative
    (state : PrefixCacheState)
    (witness : PromptCostWitness) :
    effectiveTokens state witness ≤ conservativeTokens witness := by
  cases state with
  | verifiedHit =>
      exact Nat.add_le_add_left
        (Nat.sub_le witness.promptTokens witness.cachedPromptTokens)
        witness.evidenceTokens
  | miss =>
      exact Nat.le_refl _

def exampleSemanticKey : SemanticSearchCacheKey :=
  {
    evidenceRootDigest := 11
    queryDigest := 12
    projectionVersion := 1
  }

def examplePrefixKey : ModelPrefixCacheKey :=
  {
    modelDigest := 21
    tokenizerDigest := 22
    promptPrefixDigest := 23
  }

def exampleHitContext : CostContext :=
  {
    semanticKey := exampleSemanticKey
    prefixKey := examplePrefixKey
    prefixState := .verifiedHit
    costModelVersion := 1
  }

def exampleMissContext : CostContext :=
  { exampleHitContext with prefixState := .miss }

def exampleSemanticReceipt : SemanticEvidenceReceipt :=
  {
    semanticKey := exampleSemanticKey
    validated := true
  }

def routeAWitness : PromptCostWitness :=
  {
    evidenceTokens := 0
    promptTokens := 100
    cachedPromptTokens := 90
    prefixClaimSound := by decide
  }

def routeBWitness : PromptCostWitness :=
  {
    evidenceTokens := 0
    promptTokens := 20
    cachedPromptTokens := 0
    prefixClaimSound := by decide
  }

def routeAHitCertificate : CacheBoundCostCertificate :=
  {
    claimedContext := exampleHitContext
    vector := vectorFor .verifiedHit routeAWitness 1 1 1
    witness := routeAWitness
    tokenComponentCorrect := rfl
  }

theorem route_a_hit_certificate_is_usable :
    UsableCostCertificate
      exampleHitContext
      exampleSemanticReceipt
      routeAHitCertificate := by
  simp [
    UsableCostCertificate,
    SemanticReceiptBound,
    CacheBound,
    exampleHitContext,
    exampleSemanticReceipt,
    routeAHitCertificate
  ]

theorem route_a_hit_certificate_rejected_after_miss :
    ¬ UsableCostCertificate
      exampleMissContext
      exampleSemanticReceipt
      routeAHitCertificate := by
  intro usable
  have stateBound := usable.1.2.2.1
  cases stateBound

theorem verified_hit_prefers_route_a :
    effectiveTokens .verifiedHit routeAWitness <
      effectiveTokens .verifiedHit routeBWitness := by
  decide

theorem miss_prefers_route_b :
    effectiveTokens .miss routeBWitness <
      effectiveTokens .miss routeAWitness := by
  decide

theorem route_order_flips_with_prefix_state :
    effectiveTokens .verifiedHit routeAWitness <
        effectiveTokens .verifiedHit routeBWitness ∧
      effectiveTokens .miss routeBWitness <
        effectiveTokens .miss routeAWitness := by
  exact ⟨verified_hit_prefers_route_a, miss_prefers_route_b⟩

end ASPProof.SearchRouteCacheParametricCost
