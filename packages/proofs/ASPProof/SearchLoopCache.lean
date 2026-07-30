import ASPProof.SearchLoopMerge

namespace SearchLoopCache

open SearchLoopClauseFirst

abbrev Digest := Nat
abbrev WorkspaceId := Nat
abbrev ObligationDigest := Digest
abbrev WitnessDigest := SnapshotId
abbrev ProviderArtifactDigest := Digest
abbrev SelectorDigest := Digest
abbrev RfcDigest := Digest
abbrev ProjectionVersion := Nat
abbrev BudgetClass := Nat
abbrev ModelDigest := Digest
abbrev PromptDigest := Digest

structure SemanticCacheKey where
  obligation : ObligationDigest
  workspace : WorkspaceId
  witness : WitnessDigest
  provider : ProviderId
  providerArtifact : ProviderArtifactDigest
  selector : SelectorDigest
  schema : SchemaId
  rfc : RfcDigest
  policy : PolicyId
  projection : ProjectionVersion
  budgetClass : BudgetClass
  deriving DecidableEq, Repr

def SemanticCacheKey.validationDomain
    (key : SemanticCacheKey) :
    ValidationDomain :=
  { snapshot := key.witness
    provider := key.provider
    schema := key.schema
    policy := key.policy }

structure ModelIdentity where
  model : ModelDigest
  prompt : PromptDigest
  deriving DecidableEq, Repr

inductive CacheTier where
  | semantic
  | l4
  deriving DecidableEq, Repr

structure RequestContext where
  semantic : SemanticCacheKey
  model : ModelIdentity
  deriving DecidableEq, Repr

inductive CacheKey where
  | semantic (key : SemanticCacheKey)
  | l4 (key : SemanticCacheKey) (model : ModelIdentity)
  deriving DecidableEq, Repr

def cacheKeyFor (tier : CacheTier) (context : RequestContext) : CacheKey :=
  match tier with
  | CacheTier.semantic => CacheKey.semantic context.semantic
  | CacheTier.l4 => CacheKey.l4 context.semantic context.model

inductive InvalidationReason where
  | tier
  | obligation
  | workspace
  | witness
  | provider
  | providerArtifact
  | selector
  | schema
  | rfc
  | policy
  | projection
  | budgetClass
  | model
  | prompt
  deriving DecidableEq, Repr

inductive ReuseDecision where
  | hit
  | stale (reason : InvalidationReason)
  deriving DecidableEq, Repr

def semanticMismatch
    (cached current : SemanticCacheKey) :
    InvalidationReason :=
  if cached.obligation ≠ current.obligation then
    InvalidationReason.obligation
  else if cached.workspace ≠ current.workspace then
    InvalidationReason.workspace
  else if cached.witness ≠ current.witness then
    InvalidationReason.witness
  else if cached.provider ≠ current.provider then
    InvalidationReason.provider
  else if cached.providerArtifact ≠ current.providerArtifact then
    InvalidationReason.providerArtifact
  else if cached.selector ≠ current.selector then
    InvalidationReason.selector
  else if cached.schema ≠ current.schema then
    InvalidationReason.schema
  else if cached.rfc ≠ current.rfc then
    InvalidationReason.rfc
  else if cached.policy ≠ current.policy then
    InvalidationReason.policy
  else if cached.projection ≠ current.projection then
    InvalidationReason.projection
  else if cached.budgetClass ≠ current.budgetClass then
    InvalidationReason.budgetClass
  else
    InvalidationReason.obligation

def validateSemantic
    (cached current : SemanticCacheKey) :
    ReuseDecision :=
  if cached = current then
    ReuseDecision.hit
  else
    ReuseDecision.stale (semanticMismatch cached current)

def cacheMismatch (cached current : CacheKey) : InvalidationReason :=
  match cached, current with
  | CacheKey.semantic cachedKey, CacheKey.semantic currentKey =>
      semanticMismatch cachedKey currentKey
  | CacheKey.l4 cachedKey cachedModel, CacheKey.l4 currentKey currentModel =>
      if cachedKey ≠ currentKey then
        semanticMismatch cachedKey currentKey
      else if cachedModel.model ≠ currentModel.model then
        InvalidationReason.model
      else
        InvalidationReason.prompt
  | _, _ => InvalidationReason.tier

def validateReuse (cached current : CacheKey) : ReuseDecision :=
  if cached = current then
    ReuseDecision.hit
  else
    ReuseDecision.stale (cacheMismatch cached current)

theorem validateSemantic_self (key : SemanticCacheKey) :
    validateSemantic key key = ReuseDecision.hit := by
  simp [validateSemantic]

theorem validateSemantic_hit_implies_equal
    (cached current : SemanticCacheKey)
    (hit : validateSemantic cached current = ReuseDecision.hit) :
    cached = current := by
  simpa [validateSemantic] using hit

theorem validateSemantic_hit_iff_equal
    (cached current : SemanticCacheKey) :
    validateSemantic cached current = ReuseDecision.hit ↔
      cached = current := by
  simp [validateSemantic]

theorem validateReuse_self (key : CacheKey) :
    validateReuse key key = ReuseDecision.hit := by
  simp [validateReuse]

theorem validateReuse_hit_implies_equal
    (cached current : CacheKey)
    (hit : validateReuse cached current = ReuseDecision.hit) :
    cached = current := by
  simpa [validateReuse] using hit

theorem validateReuse_hit_iff_equal
    (cached current : CacheKey) :
    validateReuse cached current = ReuseDecision.hit ↔
      cached = current := by
  simp [validateReuse]

def baseSemanticKey : SemanticCacheKey :=
  { obligation := 1
    workspace := 1
    witness := 1
    provider := 1
    providerArtifact := 1
    selector := 1
    schema := 1
    rfc := 1
    policy := 1
    projection := 1
    budgetClass := 1 }

structure LegacyCacheKey where
  obligation : ObligationDigest
  workspace : WorkspaceId
  witness : WitnessDigest
  selector : SelectorDigest
  rfc : RfcDigest
  projection : ProjectionVersion
  budgetClass : BudgetClass
  deriving DecidableEq, Repr

def legacyCacheKey (key : SemanticCacheKey) : LegacyCacheKey :=
  { obligation := key.obligation
    workspace := key.workspace
    witness := key.witness
    selector := key.selector
    rfc := key.rfc
    projection := key.projection
    budgetClass := key.budgetClass }

def providerDriftKey : SemanticCacheKey :=
  { baseSemanticKey with providerArtifact := 2 }

def policyDriftKey : SemanticCacheKey :=
  { baseSemanticKey with policy := 2 }

theorem legacy_key_collides_under_provider_artifact_drift :
    legacyCacheKey baseSemanticKey = legacyCacheKey providerDriftKey := by
  rfl

theorem provider_artifact_drift_is_typed_stale :
    validateSemantic baseSemanticKey providerDriftKey =
      ReuseDecision.stale InvalidationReason.providerArtifact := by
  rfl

theorem legacy_key_collides_under_policy_drift :
    legacyCacheKey baseSemanticKey = legacyCacheKey policyDriftKey := by
  rfl

theorem policy_drift_is_typed_stale :
    validateSemantic baseSemanticKey policyDriftKey =
      ReuseDecision.stale InvalidationReason.policy := by
  rfl

def relocateSelector
    (key : SemanticCacheKey)
    (selector : SelectorDigest) :
    SemanticCacheKey :=
  { key with selector := selector }

theorem selector_drift_is_stale_without_relocation :
    validateSemantic
        baseSemanticKey
        (relocateSelector baseSemanticKey 2) =
      ReuseDecision.stale InvalidationReason.selector := by
  rfl

theorem explicit_relocation_rekeys_to_hit
    (cached current : SemanticCacheKey)
    (relocationProof :
      relocateSelector cached current.selector = current) :
    validateSemantic
        (relocateSelector cached current.selector)
        current =
      ReuseDecision.hit := by
  rw [relocationProof]
  exact validateSemantic_self current

def baseModelIdentity : ModelIdentity :=
  { model := 1, prompt := 1 }

def baseContext : RequestContext :=
  { semantic := baseSemanticKey
    model := baseModelIdentity }

def modelDriftContext : RequestContext :=
  { baseContext with model := { baseModelIdentity with model := 2 } }

theorem model_drift_does_not_invalidate_semantic_receipt :
    cacheKeyFor CacheTier.semantic baseContext =
      cacheKeyFor CacheTier.semantic modelDriftContext := by
  rfl

theorem model_drift_invalidates_l4 :
    validateReuse
        (cacheKeyFor CacheTier.l4 baseContext)
        (cacheKeyFor CacheTier.l4 modelDriftContext) =
      ReuseDecision.stale InvalidationReason.model := by
  rfl

end SearchLoopCache
