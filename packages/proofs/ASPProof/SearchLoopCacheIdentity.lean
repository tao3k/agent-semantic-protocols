import ASPProof.SearchLoopMerge

namespace SearchLoopCacheIdentity

abbrev Digest := Nat

structure ValidationIdentity where
  sourceSnapshot : Digest
  provider : Digest
  schema : Digest
  policy : Digest
  deriving DecidableEq, Repr

structure SemanticKey where
  obligation : Digest
  workspace : Digest
  witnessSet : Digest
  domain : ValidationIdentity
  selector : Digest
  projection : Digest
  budgetClass : Digest
  deriving DecidableEq, Repr

structure ModelJudgmentKey extends SemanticKey where
  model : Digest
  prompt : Digest
  deriving DecidableEq, Repr

inductive CacheDecision where
  | reuse
  | invalidate
  deriving DecidableEq, Repr

def decideSemanticReuse (cached current : SemanticKey) : CacheDecision :=
  if cached = current then
    CacheDecision.reuse
  else
    CacheDecision.invalidate

def decideModelReuse
    (cached current : ModelJudgmentKey) :
    CacheDecision :=
  if cached = current then
    CacheDecision.reuse
  else
    CacheDecision.invalidate

theorem semantic_reuse_iff_exact_key
    (cached current : SemanticKey) :
    decideSemanticReuse cached current = CacheDecision.reuse ↔
      cached = current := by
  simp [decideSemanticReuse]

theorem stable_semantic_key_reuses (key : SemanticKey) :
    decideSemanticReuse key key = CacheDecision.reuse := by
  simp [decideSemanticReuse]

theorem semantic_key_drift_invalidates
    (cached current : SemanticKey)
    (drift : cached ≠ current) :
    decideSemanticReuse cached current = CacheDecision.invalidate := by
  simp [decideSemanticReuse, drift]

structure KeyWithoutProvider where
  obligation : Digest
  workspace : Digest
  witnessSet : Digest
  sourceSnapshot : Digest
  schema : Digest
  policy : Digest
  selector : Digest
  projection : Digest
  budgetClass : Digest
  deriving DecidableEq, Repr

def omitProvider (key : SemanticKey) : KeyWithoutProvider :=
  { obligation := key.obligation
    workspace := key.workspace
    witnessSet := key.witnessSet
    sourceSnapshot := key.domain.sourceSnapshot
    schema := key.domain.schema
    policy := key.domain.policy
    selector := key.selector
    projection := key.projection
    budgetClass := key.budgetClass }

structure KeyWithoutPolicy where
  obligation : Digest
  workspace : Digest
  witnessSet : Digest
  sourceSnapshot : Digest
  provider : Digest
  schema : Digest
  selector : Digest
  projection : Digest
  budgetClass : Digest
  deriving DecidableEq, Repr

def omitPolicy (key : SemanticKey) : KeyWithoutPolicy :=
  { obligation := key.obligation
    workspace := key.workspace
    witnessSet := key.witnessSet
    sourceSnapshot := key.domain.sourceSnapshot
    provider := key.domain.provider
    schema := key.domain.schema
    selector := key.selector
    projection := key.projection
    budgetClass := key.budgetClass }

def baseDomain : ValidationIdentity :=
  { sourceSnapshot := 1
    provider := 2
    schema := 3
    policy := 4 }

def providerDriftDomain : ValidationIdentity :=
  { sourceSnapshot := 1
    provider := 20
    schema := 3
    policy := 4 }

def policyDriftDomain : ValidationIdentity :=
  { sourceSnapshot := 1
    provider := 2
    schema := 3
    policy := 40 }

def baseSemanticKey : SemanticKey :=
  { obligation := 10
    workspace := 11
    witnessSet := 12
    domain := baseDomain
    selector := 13
    projection := 14
    budgetClass := 15 }

def providerDriftKey : SemanticKey :=
  { baseSemanticKey with domain := providerDriftDomain }

def policyDriftKey : SemanticKey :=
  { baseSemanticKey with domain := policyDriftDomain }

theorem omitting_provider_allows_false_reuse :
    omitProvider baseSemanticKey = omitProvider providerDriftKey ∧
      decideSemanticReuse baseSemanticKey providerDriftKey =
        CacheDecision.invalidate := by
  decide

theorem omitting_policy_allows_false_reuse :
    omitPolicy baseSemanticKey = omitPolicy policyDriftKey ∧
      decideSemanticReuse baseSemanticKey policyDriftKey =
        CacheDecision.invalidate := by
  decide

structure CacheState where
  semanticKey : SemanticKey
  workspaceGeneration : Digest
  deriving DecidableEq, Repr

def generationCompatible (cached current : CacheState) : Prop :=
  cached.workspaceGeneration = current.workspaceGeneration

def baseCacheState : CacheState :=
  { semanticKey := baseSemanticKey
    workspaceGeneration := 100 }

def unrelatedGenerationChange : CacheState :=
  { semanticKey := baseSemanticKey
    workspaceGeneration := 101 }

theorem whole_generation_causes_false_invalidation :
    ¬ generationCompatible baseCacheState unrelatedGenerationChange ∧
      decideSemanticReuse
          baseCacheState.semanticKey
          unrelatedGenerationChange.semanticKey =
        CacheDecision.reuse := by
  simp
    [ generationCompatible
    , baseCacheState
    , unrelatedGenerationChange
    , decideSemanticReuse
    ]

def firstModelKey : ModelJudgmentKey :=
  { toSemanticKey := baseSemanticKey
    model := 200
    prompt := 201 }

def secondModelKey : ModelJudgmentKey :=
  { toSemanticKey := baseSemanticKey
    model := 202
    prompt := 203 }

theorem model_drift_invalidates_l4 :
    decideModelReuse firstModelKey secondModelKey =
      CacheDecision.invalidate := by
  decide

theorem model_drift_preserves_semantic_reuse :
    decideSemanticReuse
        firstModelKey.toSemanticKey
        secondModelKey.toSemanticKey =
      CacheDecision.reuse := by
  decide

inductive EntryState where
  | valid
  | stale
  deriving DecidableEq, Repr

structure CacheEntry where
  key : SemanticKey
  state : EntryState
  deriving DecidableEq, Repr

def reconcile
    (current : SemanticKey)
    (entry : CacheEntry) :
    CacheEntry :=
  if entry.key = current then
    { entry with state := EntryState.valid }
  else
    { entry with state := EntryState.stale }

theorem reconcile_drift_is_stale
    (current : SemanticKey)
    (entry : CacheEntry)
    (drift : entry.key ≠ current) :
    (reconcile current entry).state = EntryState.stale := by
  simp [reconcile, drift]

theorem reconciled_valid_implies_exact_key
    (current : SemanticKey)
    (entry : CacheEntry)
    (valid : (reconcile current entry).state = EntryState.valid) :
    entry.key = current := by
  by_cases exactKey : entry.key = current
  · exact exactKey
  · simp [reconcile, exactKey] at valid

end SearchLoopCacheIdentity
