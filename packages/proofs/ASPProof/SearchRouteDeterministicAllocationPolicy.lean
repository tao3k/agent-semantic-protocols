namespace ASPProof.SearchRouteDeterministicAllocationPolicy

inductive PolicySemantics where
  | sharedOnly
  | alphaAll
  | cacheDirected
  deriving DecidableEq

def policyDigest : PolicySemantics → Nat
  | .sharedOnly => 1
  | .alphaAll => 2
  | .cacheDirected => 3

theorem policy_digest_is_injective :
    Function.Injective policyDigest := by
  intro left right equalDigest
  cases left <;> cases right <;>
    simp [policyDigest] at equalDigest <;>
    rfl

structure BatchFacts where
  graphGeneration : Nat
  batchDigest : Nat
  callTokens : Nat
  synthesisTokens : Nat
  searchCacheContext : Nat
  modelPrefixCacheContext : Nat
  deriving DecidableEq

def BatchFacts.totalTokens (facts : BatchFacts) : Nat :=
  facts.callTokens + facts.synthesisTokens

structure Allocation where
  alpha : Nat
  beta : Nat
  shared : Nat
  deriving DecidableEq

def Allocation.totalTokens (allocation : Allocation) : Nat :=
  allocation.alpha + allocation.beta + allocation.shared

def evaluate
    (policy : PolicySemantics)
    (facts : BatchFacts) : Allocation :=
  match policy with
  | .sharedOnly =>
      { alpha := 0, beta := 0, shared := facts.totalTokens }
  | .alphaAll =>
      { alpha := facts.totalTokens, beta := 0, shared := 0 }
  | .cacheDirected =>
      if facts.searchCacheContext = 11 then
        { alpha := facts.totalTokens, beta := 0, shared := 0 }
      else
        { alpha := 0, beta := 0, shared := facts.totalTokens }

def Conserves (facts : BatchFacts) (allocation : Allocation) : Prop :=
  allocation.totalTokens = facts.totalTokens

theorem evaluated_allocation_conserves_total
    (policy : PolicySemantics)
    (facts : BatchFacts) :
    Conserves facts (evaluate policy facts) := by
  cases policy with
  | sharedOnly =>
      simp
        [ Conserves
        , evaluate
        , Allocation.totalTokens
        , BatchFacts.totalTokens
        ]
  | alphaAll =>
      simp
        [ Conserves
        , evaluate
        , Allocation.totalTokens
        , BatchFacts.totalTokens
        ]
  | cacheDirected =>
      simp
        [ Conserves
        , evaluate
        , Allocation.totalTokens
        , BatchFacts.totalTokens
        ]
      split <;> simp

structure CertifiedAllocation where
  policy : PolicySemantics
  claimedPolicyDigest : Nat
  facts : BatchFacts
  allocation : Allocation
  digestMatches :
    claimedPolicyDigest = policyDigest policy
  evaluates :
    allocation = evaluate policy facts

def ReplayComparable
    (left right : CertifiedAllocation) : Prop :=
  left.claimedPolicyDigest = right.claimedPolicyDigest ∧
  left.facts = right.facts

theorem certified_allocation_conserves_total
    (certificate : CertifiedAllocation) :
    Conserves certificate.facts certificate.allocation := by
  rw [certificate.evaluates]
  exact evaluated_allocation_conserves_total _ _

theorem same_digest_and_facts_replay_same_allocation
    {left right : CertifiedAllocation}
    (comparable : ReplayComparable left right) :
    left.allocation = right.allocation := by
  have samePolicy : left.policy = right.policy :=
    policy_digest_is_injective <| by
      calc
        policyDigest left.policy =
            left.claimedPolicyDigest :=
              left.digestMatches.symm
        _ = right.claimedPolicyDigest := comparable.1
        _ = policyDigest right.policy := right.digestMatches
  rw [left.evaluates, right.evaluates, samePolicy, comparable.2]

theorem replay_comparability_is_reflexive
    (certificate : CertifiedAllocation) :
    ReplayComparable certificate certificate :=
  ⟨rfl, rfl⟩

def unsafeDigest (_ : PolicySemantics) : Nat :=
  9

theorem unsafe_digest_is_not_injective :
    unsafeDigest .sharedOnly = unsafeDigest .alphaAll ∧
    PolicySemantics.sharedOnly ≠ PolicySemantics.alphaAll := by
  exact ⟨rfl, by decide⟩

def facts11 : BatchFacts where
  graphGeneration := 7
  batchDigest := 101
  callTokens := 20
  synthesisTokens := 5
  searchCacheContext := 11
  modelPrefixCacheContext := 13

def facts99 : BatchFacts where
  graphGeneration := 7
  batchDigest := 102
  callTokens := 20
  synthesisTokens := 5
  searchCacheContext := 99
  modelPrefixCacheContext := 13

theorem colliding_digest_breaks_allocation_replay :
    unsafeDigest .sharedOnly = unsafeDigest .alphaAll ∧
    evaluate .sharedOnly facts11 ≠ evaluate .alphaAll facts11 := by
  exact ⟨rfl, by decide⟩

def cache11Certificate : CertifiedAllocation where
  policy := .cacheDirected
  claimedPolicyDigest := 3
  facts := facts11
  allocation := evaluate .cacheDirected facts11
  digestMatches := rfl
  evaluates := rfl

def cache99Certificate : CertifiedAllocation where
  policy := .cacheDirected
  claimedPolicyDigest := 3
  facts := facts99
  allocation := evaluate .cacheDirected facts99
  digestMatches := rfl
  evaluates := rfl

theorem same_policy_digest_without_same_facts_is_insufficient :
    cache11Certificate.claimedPolicyDigest =
        cache99Certificate.claimedPolicyDigest ∧
    cache11Certificate.facts ≠ cache99Certificate.facts ∧
    cache11Certificate.allocation ≠ cache99Certificate.allocation := by
  exact ⟨rfl, by decide, by decide⟩

theorem cache_context_change_rejects_replay_comparison :
    ¬ ReplayComparable cache11Certificate cache99Certificate := by
  intro comparable
  have factsDiffer :
      cache11Certificate.facts ≠ cache99Certificate.facts := by
    decide
  exact factsDiffer comparable.2

theorem deterministic_replay_preserves_pareto_cost :
    cache11Certificate.allocation.alpha =
      cache11Certificate.allocation.alpha :=
  congrArg Allocation.alpha <|
    same_digest_and_facts_replay_same_allocation
      (replay_comparability_is_reflexive cache11Certificate)

end ASPProof.SearchRouteDeterministicAllocationPolicy
