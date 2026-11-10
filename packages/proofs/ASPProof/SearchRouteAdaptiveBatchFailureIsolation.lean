-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteBatchClaimDagEquivalence

namespace ASPProof.SearchRouteAdaptiveBatchFailureIsolation

open ASPProof.SearchRouteBatchClaimDagEquivalence

structure RoutedClaim where
  claimId : Nat
  key : BatchExecutionKey
  deriving DecidableEq, Repr

structure CompatibleBatch where
  key : BatchExecutionKey
  claims : List RoutedClaim
  deriving DecidableEq, Repr

def BatchCompatible (batch : CompatibleBatch) : Prop :=
  batch.claims ≠ [] ∧
  ∀ claim ∈ batch.claims, claim.key = batch.key

structure PartitionPlan where
  source : List RoutedClaim
  batches : List CompatibleBatch
  deriving DecidableEq, Repr

def partitionClaims (batches : List CompatibleBatch) : List RoutedClaim :=
  batches.flatMap CompatibleBatch.claims

def ValidPartitionPlan (plan : PartitionPlan) : Prop :=
  plan.source ≠ [] ∧
  plan.source.Nodup ∧
  plan.batches ≠ [] ∧
  (∀ batch ∈ plan.batches, BatchCompatible batch) ∧
  partitionClaims plan.batches = plan.source

def SharesExecution (plan : PartitionPlan) : Prop :=
  plan.batches.length < plan.source.length

structure BatchAttempt where
  batch : CompatibleBatch
  succeeded : Bool
  deriving DecidableEq, Repr

def AttemptsMatchPlan (plan : PartitionPlan) (attempts : List BatchAttempt) : Prop :=
  attempts.map BatchAttempt.batch = plan.batches

def claimSucceededInAttempts
    (attempts : List BatchAttempt)
    (claim : RoutedClaim) : Bool :=
  attempts.any fun attempt =>
    attempt.succeeded && attempt.batch.claims.contains claim

def successfulClaims
    (source : List RoutedClaim)
    (attempts : List BatchAttempt) : List RoutedClaim :=
  source.filter (claimSucceededInAttempts attempts)

def unresolvedClaims
    (source : List RoutedClaim)
    (attempts : List BatchAttempt) : List RoutedClaim :=
  source.filter fun claim => !(claimSucceededInAttempts attempts claim)

theorem source_claim_is_successful_or_unresolved
    {source : List RoutedClaim}
    {attempts : List BatchAttempt}
    {claim : RoutedClaim}
    (hclaim : claim ∈ source) :
    claim ∈ successfulClaims source attempts ∨
      claim ∈ unresolvedClaims source attempts := by
  cases hstatus : claimSucceededInAttempts attempts claim <;>
    simp [successfulClaims, unresolvedClaims, hclaim, hstatus]

theorem successful_claim_is_not_unresolved
    {source : List RoutedClaim}
    {attempts : List BatchAttempt}
    {claim : RoutedClaim}
    (hsuccess : claim ∈ successfulClaims source attempts) :
    claim ∉ unresolvedClaims source attempts := by
  simp only [successfulClaims, List.mem_filter] at hsuccess
  simp [unresolvedClaims, hsuccess.1, hsuccess.2]

inductive FailureReason where
  | batchTransportUnavailable
  | batchResourceLimit
  | semanticRejection
  | policyMismatch
  | replayMismatch
  | agentPreference
  deriving DecidableEq, Repr

def FallbackEligibleReason : FailureReason → Bool
  | .batchTransportUnavailable => true
  | .batchResourceLimit => true
  | .semanticRejection => false
  | .policyMismatch => false
  | .replayMismatch => false
  | .agentPreference => false

structure FailureAnnotation where
  claim : RoutedClaim
  reason : FailureReason
  deriving DecidableEq, Repr

def annotationsMatchUnresolved
    (source : List RoutedClaim)
    (attempts : List BatchAttempt)
    (annotations : List FailureAnnotation) : Prop :=
  annotations.map FailureAnnotation.claim = unresolvedClaims source attempts

def eligibleFallbackClaims
    (annotations : List FailureAnnotation) : List RoutedClaim :=
  (annotations.filter fun annotation => FallbackEligibleReason annotation.reason).map
    FailureAnnotation.claim

def failClosedClaims
    (annotations : List FailureAnnotation) : List RoutedClaim :=
  (annotations.filter fun annotation => !(FallbackEligibleReason annotation.reason)).map
    FailureAnnotation.claim

theorem annotation_claim_is_eligible_or_fail_closed
    {annotations : List FailureAnnotation}
    {annotation : FailureAnnotation}
    (hannotation : annotation ∈ annotations) :
    annotation.claim ∈ eligibleFallbackClaims annotations ∨
      annotation.claim ∈ failClosedClaims annotations := by
  cases heligible : FallbackEligibleReason annotation.reason with
  | false =>
      right
      simp only [failClosedClaims, List.mem_map]
      refine ⟨annotation, ?_, rfl⟩
      simp [hannotation, heligible]
  | true =>
      left
      simp only [eligibleFallbackClaims, List.mem_map]
      refine ⟨annotation, ?_, rfl⟩
      simp [hannotation, heligible]

theorem unresolved_claim_is_eligible_or_fail_closed
    {source : List RoutedClaim}
    {attempts : List BatchAttempt}
    {annotations : List FailureAnnotation}
    (hmatch : annotationsMatchUnresolved source attempts annotations)
    {claim : RoutedClaim}
    (hunresolved : claim ∈ unresolvedClaims source attempts) :
    claim ∈ eligibleFallbackClaims annotations ∨ claim ∈ failClosedClaims annotations := by
  have hmapped : claim ∈ annotations.map FailureAnnotation.claim := by
    rw [hmatch]
    exact hunresolved
  simp only [List.mem_map] at hmapped
  rcases hmapped with ⟨annotation, hannotation, rfl⟩
  exact annotation_claim_is_eligible_or_fail_closed hannotation

theorem semantic_rejection_is_not_fallback_eligible :
    FallbackEligibleReason .semanticRejection = false := by
  rfl

theorem policy_mismatch_is_not_fallback_eligible :
    FallbackEligibleReason .policyMismatch = false := by
  rfl

theorem replay_mismatch_is_not_fallback_eligible :
    FallbackEligibleReason .replayMismatch = false := by
  rfl

theorem agent_preference_is_not_fallback_eligible :
    FallbackEligibleReason .agentPreference = false := by
  rfl

structure FallbackBudget where
  maxClaims : Nat
  maxAdditionalRounds : Nat
  maxAdditionalTokens : Nat
  deriving DecidableEq, Repr

structure FallbackReceipt where
  sourceDigest : Nat
  attemptSetDigest : Nat
  checkpointDigest : Nat
  claims : List RoutedClaim
  deriving DecidableEq, Repr

def ValidFallbackReceipt
    (expectedSourceDigest expectedAttemptSetDigest expectedCheckpointDigest : Nat)
    (sharedTokens perClaimTokens : Nat)
    (source : List RoutedClaim)
    (attempts : List BatchAttempt)
    (annotations : List FailureAnnotation)
    (budget : FallbackBudget)
    (receipt : FallbackReceipt) : Prop :=
  receipt.sourceDigest = expectedSourceDigest ∧
  receipt.attemptSetDigest = expectedAttemptSetDigest ∧
  receipt.checkpointDigest = expectedCheckpointDigest ∧
  annotationsMatchUnresolved source attempts annotations ∧
  receipt.claims = eligibleFallbackClaims annotations ∧
  receipt.claims.length ≤ budget.maxClaims ∧
  2 * receipt.claims.length ≤ budget.maxAdditionalRounds ∧
  receipt.claims.length * (sharedTokens + perClaimTokens) ≤ budget.maxAdditionalTokens

theorem valid_fallback_contains_only_eligible_annotations
    {source : List RoutedClaim}
    {attempts : List BatchAttempt}
    {annotations : List FailureAnnotation}
    {budget : FallbackBudget}
    {receipt : FallbackReceipt}
    {sourceDigest attemptSetDigest checkpointDigest sharedTokens perClaimTokens : Nat}
    (hvalid : ValidFallbackReceipt sourceDigest attemptSetDigest checkpointDigest
      sharedTokens perClaimTokens source attempts annotations budget receipt)
    {claim : RoutedClaim}
    (hclaim : claim ∈ receipt.claims) :
    ∃ annotation ∈ annotations,
      annotation.claim = claim ∧ FallbackEligibleReason annotation.reason = true := by
  rw [hvalid.2.2.2.2.1] at hclaim
  simp only [eligibleFallbackClaims, List.mem_map, List.mem_filter] at hclaim
  rcases hclaim with ⟨annotation, ⟨heligible, hannotation⟩, hclaim⟩
  exact ⟨annotation, heligible, hclaim, hannotation⟩

theorem valid_fallback_covers_every_eligible_claim
    {source : List RoutedClaim}
    {attempts : List BatchAttempt}
    {annotations : List FailureAnnotation}
    {budget : FallbackBudget}
    {receipt : FallbackReceipt}
    {sourceDigest attemptSetDigest checkpointDigest sharedTokens perClaimTokens : Nat}
    (hvalid : ValidFallbackReceipt sourceDigest attemptSetDigest checkpointDigest
      sharedTokens perClaimTokens source attempts annotations budget receipt)
    {claim : RoutedClaim}
    (heligible : claim ∈ eligibleFallbackClaims annotations) :
    claim ∈ receipt.claims := by
  rw [hvalid.2.2.2.2.1]
  exact heligible

theorem every_source_claim_is_success_fallback_or_fail_closed
    {source : List RoutedClaim}
    {attempts : List BatchAttempt}
    {annotations : List FailureAnnotation}
    {budget : FallbackBudget}
    {receipt : FallbackReceipt}
    {sourceDigest attemptSetDigest checkpointDigest sharedTokens perClaimTokens : Nat}
    (hvalid : ValidFallbackReceipt sourceDigest attemptSetDigest checkpointDigest
      sharedTokens perClaimTokens source attempts annotations budget receipt)
    {claim : RoutedClaim}
    (hclaim : claim ∈ source) :
    claim ∈ successfulClaims source attempts ∨
      claim ∈ receipt.claims ∨
      claim ∈ failClosedClaims annotations := by
  rcases source_claim_is_successful_or_unresolved hclaim with hsuccess | hunresolved
  · exact Or.inl hsuccess
  · rcases unresolved_claim_is_eligible_or_fail_closed hvalid.2.2.2.1 hunresolved with
      heligible | hclosed
    · exact Or.inr (Or.inl (valid_fallback_covers_every_eligible_claim hvalid heligible))
    · exact Or.inr (Or.inr hclosed)

def partitionAttemptRounds (batchCount : Nat) : Nat :=
  2 * batchCount

def individualFallbackRounds (unresolvedCount : Nat) : Nat :=
  2 * unresolvedCount

def adaptiveToolRounds (batchCount unresolvedCount : Nat) : Nat :=
  partitionAttemptRounds batchCount + individualFallbackRounds unresolvedCount

theorem adaptive_tool_rounds_le_individual_iff
    (claimCount batchCount unresolvedCount : Nat) :
    adaptiveToolRounds batchCount unresolvedCount ≤ individualToolRounds claimCount ↔
      batchCount + unresolvedCount ≤ claimCount := by
  change 2 * batchCount + 2 * unresolvedCount ≤ 2 * claimCount ↔
    batchCount + unresolvedCount ≤ claimCount
  rw [← Nat.mul_add]
  exact Nat.mul_le_mul_left_iff (by decide)

theorem adaptive_tool_rounds_save_one_claim_iff
    (claimCount batchCount unresolvedCount : Nat) :
    adaptiveToolRounds batchCount unresolvedCount + 2 ≤
        individualToolRounds claimCount ↔
      batchCount + unresolvedCount + 1 ≤ claimCount := by
  change 2 * batchCount + 2 * unresolvedCount + 2 ≤ 2 * claimCount ↔
    batchCount + unresolvedCount + 1 ≤ claimCount
  have hnormalize :
      2 * batchCount + 2 * unresolvedCount + 2 =
        2 * (batchCount + unresolvedCount + 1) := by
    simp [Nat.mul_add]
  rw [hnormalize]
  exact Nat.mul_le_mul_left_iff (by decide)

theorem no_failure_partition_saves_rounds
    {claimCount batchCount : Nat}
    (hbatch : batchCount + 1 ≤ claimCount) :
    adaptiveToolRounds batchCount 0 + 2 ≤ individualToolRounds claimCount := by
  rw [adaptive_tool_rounds_save_one_claim_iff]
  simpa using hbatch

theorem batch_count_bound_alone_does_not_prove_round_savings :
    ∃ claimCount batchCount unresolvedCount,
      0 < claimCount ∧
      batchCount ≤ claimCount ∧
      individualToolRounds claimCount < adaptiveToolRounds batchCount unresolvedCount := by
  refine ⟨2, 1, 2, by decide, by decide, ?_⟩
  decide

theorem singleton_partition_without_fallback_has_no_round_savings
    (claimCount : Nat) :
    adaptiveToolRounds claimCount 0 = individualToolRounds claimCount := by
  simp [adaptiveToolRounds, partitionAttemptRounds, individualFallbackRounds,
    individualToolRounds]

theorem singleton_partition_with_any_fallback_regresses
    {claimCount unresolvedCount : Nat}
    (hunresolved : 0 < unresolvedCount) :
    individualToolRounds claimCount <
      adaptiveToolRounds claimCount unresolvedCount := by
  simp [adaptiveToolRounds, partitionAttemptRounds, individualFallbackRounds,
    individualToolRounds]
  omega

theorem full_fallback_cannot_satisfy_round_nonregression
    {claimCount batchCount : Nat}
    (hbatch : 0 < batchCount) :
    ¬ adaptiveToolRounds batchCount claimCount ≤ individualToolRounds claimCount := by
  rw [adaptive_tool_rounds_le_individual_iff]
  intro hregression
  exact (Nat.not_lt_of_ge hregression) (Nat.lt_add_of_pos_left hbatch)

theorem single_batch_plus_full_fallback_always_costs_more
    (claimCount : Nat) :
    individualToolRounds claimCount < adaptiveToolRounds 1 claimCount := by
  simp [adaptiveToolRounds, partitionAttemptRounds, individualFallbackRounds,
    individualToolRounds]

def partitionAttemptInputTokens
    (sharedTokens perClaimTokens claimCount batchCount : Nat) : Nat :=
  batchCount * sharedTokens + claimCount * perClaimTokens

def fallbackInputTokens
    (sharedTokens perClaimTokens unresolvedCount : Nat) : Nat :=
  unresolvedCount * (sharedTokens + perClaimTokens)

def adaptiveInputTokens
    (sharedTokens perClaimTokens claimCount batchCount unresolvedCount : Nat) : Nat :=
  partitionAttemptInputTokens sharedTokens perClaimTokens claimCount batchCount +
    fallbackInputTokens sharedTokens perClaimTokens unresolvedCount

theorem adaptive_input_tokens_le_individual_iff
    (sharedTokens perClaimTokens claimCount batchCount unresolvedCount : Nat) :
    adaptiveInputTokens sharedTokens perClaimTokens claimCount batchCount unresolvedCount ≤
        individualInputTokens sharedTokens perClaimTokens claimCount ↔
      (batchCount + unresolvedCount) * sharedTokens +
          unresolvedCount * perClaimTokens ≤ claimCount * sharedTokens := by
  have hadaptive :
      adaptiveInputTokens sharedTokens perClaimTokens claimCount batchCount unresolvedCount =
        ((batchCount + unresolvedCount) * sharedTokens +
          unresolvedCount * perClaimTokens) + claimCount * perClaimTokens := by
    simp [adaptiveInputTokens, partitionAttemptInputTokens, fallbackInputTokens,
      Nat.mul_add, Nat.add_mul]
    omega
  have hindividual :
      individualInputTokens sharedTokens perClaimTokens claimCount =
        claimCount * sharedTokens + claimCount * perClaimTokens := by
    simp [individualInputTokens, Nat.mul_add]
  rw [hadaptive, hindividual]
  exact Nat.add_le_add_iff_right

def CostAdmissible
    (sharedTokens perClaimTokens claimCount batchCount unresolvedCount : Nat) : Prop :=
  adaptiveToolRounds batchCount unresolvedCount ≤ individualToolRounds claimCount ∧
  adaptiveInputTokens sharedTokens perClaimTokens claimCount batchCount unresolvedCount ≤
    individualInputTokens sharedTokens perClaimTokens claimCount

structure AdmittedAdaptiveExecution
    (sourceDigest attemptSetDigest checkpointDigest sharedTokens perClaimTokens : Nat)
    (plan : PartitionPlan)
    (attempts : List BatchAttempt)
    (annotations : List FailureAnnotation)
    (budget : FallbackBudget)
    (receipt : FallbackReceipt) : Prop where
  partition : ValidPartitionPlan plan
  attemptsMatch : AttemptsMatchPlan plan attempts
  fallback : ValidFallbackReceipt sourceDigest attemptSetDigest checkpointDigest
    sharedTokens perClaimTokens plan.source attempts annotations budget receipt
  sharesExecution : SharesExecution plan
  cost : CostAdmissible sharedTokens perClaimTokens plan.source.length
    plan.batches.length receipt.claims.length

theorem cost_admission_implies_round_and_token_bounds
    {sharedTokens perClaimTokens claimCount batchCount unresolvedCount : Nat}
    (hadmit : CostAdmissible sharedTokens perClaimTokens claimCount
      batchCount unresolvedCount) :
    adaptiveToolRounds batchCount unresolvedCount ≤ individualToolRounds claimCount ∧
    adaptiveInputTokens sharedTokens perClaimTokens claimCount batchCount unresolvedCount ≤
      individualInputTokens sharedTokens perClaimTokens claimCount :=
  hadmit

theorem admitted_execution_cannot_fallback_every_source_claim
    {sourceDigest attemptSetDigest checkpointDigest sharedTokens perClaimTokens : Nat}
    {plan : PartitionPlan}
    {attempts : List BatchAttempt}
    {annotations : List FailureAnnotation}
    {budget : FallbackBudget}
    {receipt : FallbackReceipt}
    (hadmit : AdmittedAdaptiveExecution sourceDigest attemptSetDigest checkpointDigest
      sharedTokens perClaimTokens plan attempts annotations budget receipt) :
    receipt.claims.length ≠ plan.source.length := by
  intro hall
  have hround := hadmit.cost.1
  rw [hall] at hround
  cases hbatchEq : plan.batches with
  | nil => exact hadmit.partition.2.2.1 hbatchEq
  | cons head tail =>
      have hbatches : 0 < plan.batches.length := by
        simp [hbatchEq]
      exact full_fallback_cannot_satisfy_round_nonregression hbatches hround

structure FallbackCacheIdentity where
  sourceDigest : Nat
  attemptSetDigest : Nat
  unresolvedClaimsDigest : Nat
  checkpointDigest : Nat
  deriving DecidableEq, Repr

def PlanOnlyFallbackCacheHit
    (left right : FallbackCacheIdentity) : Prop :=
  left.sourceDigest = right.sourceDigest

def FallbackCacheReusable
    (left right : FallbackCacheIdentity) : Prop :=
  left = right

def exampleFallbackCache : FallbackCacheIdentity :=
  { sourceDigest := 101
    attemptSetDigest := 201
    unresolvedClaimsDigest := 301
    checkpointDigest := 401 }

def staleAttemptFallbackCache : FallbackCacheIdentity :=
  { sourceDigest := 101
    attemptSetDigest := 202
    unresolvedClaimsDigest := 302
    checkpointDigest := 401 }

theorem plan_only_cache_hit_does_not_prove_fallback_reuse :
    PlanOnlyFallbackCacheHit exampleFallbackCache staleAttemptFallbackCache ∧
    ¬ FallbackCacheReusable exampleFallbackCache staleAttemptFallbackCache := by
  simp [PlanOnlyFallbackCacheHit, FallbackCacheReusable, exampleFallbackCache,
    staleAttemptFallbackCache]

def keyA : BatchExecutionKey :=
  { policyDigest := 10
    serializerDigest := 20
    statementSchemaDigest := 30
    verifierIdDigest := 40
    verifierBinaryDigest := 50
    decoderSchemaDigest := 60 }

def keyB : BatchExecutionKey :=
  { policyDigest := 11
    serializerDigest := 20
    statementSchemaDigest := 30
    verifierIdDigest := 40
    verifierBinaryDigest := 50
    decoderSchemaDigest := 60 }

def claimA1 : RoutedClaim := { claimId := 1, key := keyA }
def claimA2 : RoutedClaim := { claimId := 2, key := keyA }
def claimB : RoutedClaim := { claimId := 3, key := keyB }

def batchA : CompatibleBatch := { key := keyA, claims := [claimA1, claimA2] }
def batchB : CompatibleBatch := { key := keyB, claims := [claimB] }
def mixedKeyBatch : CompatibleBatch := { key := keyA, claims := [claimA1, claimB] }

def examplePlan : PartitionPlan :=
  { source := [claimA1, claimA2, claimB]
    batches := [batchA, batchB] }

def exampleAttempts : List BatchAttempt :=
  [ { batch := batchA, succeeded := true },
    { batch := batchB, succeeded := false } ]

def exampleFallbackReceipt : FallbackReceipt :=
  { sourceDigest := 1001
    attemptSetDigest := 1002
    checkpointDigest := 1003
    claims := [claimB] }

def exampleFailureAnnotations : List FailureAnnotation :=
  [ { claim := claimB, reason := .batchTransportUnavailable } ]

def exampleFallbackBudget : FallbackBudget :=
  { maxClaims := 1
    maxAdditionalRounds := 2
    maxAdditionalTokens := 5 }

def semanticRejectionAnnotations : List FailureAnnotation :=
  [ { claim := claimB, reason := .semanticRejection } ]

def abusiveSemanticFallbackReceipt : FallbackReceipt :=
  { sourceDigest := 1001
    attemptSetDigest := 1002
    checkpointDigest := 1003
    claims := [claimB] }

theorem example_partition_is_valid : ValidPartitionPlan examplePlan := by
  simp [ValidPartitionPlan, examplePlan, partitionClaims, BatchCompatible,
    batchA, batchB, claimA1, claimA2, claimB, keyA, keyB]

theorem example_partition_shares_execution : SharesExecution examplePlan := by
  simp [SharesExecution, examplePlan]

theorem cross_key_batch_is_rejected : ¬ BatchCompatible mixedKeyBatch := by
  simp [BatchCompatible, mixedKeyBatch, claimA1, claimB, keyA, keyB]

theorem example_attempts_match_partition : AttemptsMatchPlan examplePlan exampleAttempts := by
  simp [AttemptsMatchPlan, examplePlan, exampleAttempts]

theorem example_success_isolated_from_failed_batch :
    successfulClaims examplePlan.source exampleAttempts = [claimA1, claimA2] ∧
    unresolvedClaims examplePlan.source exampleAttempts = [claimB] := by
  decide

theorem example_fallback_is_exact :
    ValidFallbackReceipt 1001 1002 1003 3 2 examplePlan.source
      exampleAttempts exampleFailureAnnotations exampleFallbackBudget
      exampleFallbackReceipt := by
  simp [ValidFallbackReceipt, annotationsMatchUnresolved, eligibleFallbackClaims,
    FallbackEligibleReason,
    exampleFallbackReceipt, exampleFallbackBudget, exampleFailureAnnotations,
    examplePlan, exampleAttempts, unresolvedClaims, claimSucceededInAttempts,
    batchA, batchB, claimA1, claimA2, claimB, keyA, keyB]

theorem semantic_rejection_fallback_is_rejected :
    ¬ ValidFallbackReceipt 1001 1002 1003 3 2 examplePlan.source
      exampleAttempts semanticRejectionAnnotations exampleFallbackBudget
      abusiveSemanticFallbackReceipt := by
  simp [ValidFallbackReceipt, annotationsMatchUnresolved, eligibleFallbackClaims,
    FallbackEligibleReason,
    abusiveSemanticFallbackReceipt, exampleFallbackBudget, semanticRejectionAnnotations,
    examplePlan, exampleAttempts, unresolvedClaims, claimSucceededInAttempts,
    batchA, batchB, claimA1, claimA2, claimB, keyA, keyB]

end ASPProof.SearchRouteAdaptiveBatchFailureIsolation
