-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteAdmissionRetryCacheRejoinBoundedCandidateAdmissionCost

namespace ASPProof.SearchRouteAdmissionRetryCacheRejoinProjectionByteTokenCacheBoundary

def EncodedProjectionBytes
    (fixedBytes candidateCount candidateReceiptBytes
      comparisonCount comparisonReceiptBytes : Nat) : Nat :=
  fixedBytes
    + candidateCount * candidateReceiptBytes
    + comparisonCount * comparisonReceiptBytes

structure TokenizerUpperBound where
  count : Nat → Nat
  overhead : Nat
  tokensPerEncodedByte : Nat
  bound :
    ∀ encodedBytes,
      count encodedBytes
        ≤ overhead + encodedBytes * tokensPerEncodedByte

def ProjectedPromptTokens
    (tokenizer : TokenizerUpperBound)
    (promptFixedTokens encodedBytes : Nat) : Nat :=
  promptFixedTokens + tokenizer.count encodedBytes

def EscapedJsonStringBytes (rawBytes : Nat) : Nat :=
  Nat.succ rawBytes + (5 * rawBytes + 1)

structure ProjectionIdentity where
  graphDigest : Nat
  rendererVersion : Nat
  projectionPolicyDigest : Nat
  schemaDigest : Nat
  deriving DecidableEq

def ProjectionCompatible
    (left right : ProjectionIdentity) : Prop :=
  left = right

structure SearchResultCacheKey where
  workspaceSnapshotDigest : Nat
  queryDigest : Nat
  searchPolicyDigest : Nat
  providerSchemaDigest : Nat
  deriving DecidableEq

def SearchResultCacheCompatible
    (left right : SearchResultCacheKey) : Prop :=
  left = right

structure ModelPrefixCacheKey where
  modelDigest : Nat
  systemPromptDigest : Nat
  toolSchemaDigest : Nat
  renderedPrefixDigest : Nat
  prefixBoundaryDigest : Nat
  deriving DecidableEq

def ModelPrefixCacheCompatible
    (left right : ModelPrefixCacheKey) : Prop :=
  left = right

theorem zero_candidate_projection_has_only_fixed_bytes
    (fixedBytes candidateReceiptBytes comparisonReceiptBytes : Nat) :
    EncodedProjectionBytes
      fixedBytes 0 candidateReceiptBytes 0 comparisonReceiptBytes
      = fixedBytes := by
  unfold EncodedProjectionBytes
  rw [Nat.zero_mul, Nat.zero_mul, Nat.add_zero]

theorem projection_bytes_are_bounded_by_candidate_capacity
    (fixedBytes candidateCount candidateCapacity candidateReceiptBytes
      comparisonCount comparisonReceiptBytes : Nat)
    (candidateBound : candidateCount ≤ candidateCapacity)
    (comparisonBound : comparisonCount ≤ candidateCount) :
    EncodedProjectionBytes
        fixedBytes
        candidateCount
        candidateReceiptBytes
        comparisonCount
        comparisonReceiptBytes
      ≤
    EncodedProjectionBytes
        fixedBytes
        candidateCapacity
        candidateReceiptBytes
        candidateCapacity
        comparisonReceiptBytes := by
  unfold EncodedProjectionBytes
  apply Nat.add_le_add
  · exact Nat.add_le_add_left
      (Nat.mul_le_mul_right candidateReceiptBytes candidateBound)
      fixedBytes
  · exact Nat.mul_le_mul_right comparisonReceiptBytes
      (Nat.le_trans comparisonBound candidateBound)

theorem projected_tokens_are_bounded_by_encoded_bytes
    (tokenizer : TokenizerUpperBound)
    (promptFixedTokens encodedBytes encodedByteCapacity : Nat)
    (encodedBound : encodedBytes ≤ encodedByteCapacity) :
    ProjectedPromptTokens tokenizer promptFixedTokens encodedBytes
      ≤
    promptFixedTokens
      + (tokenizer.overhead
        + encodedByteCapacity * tokenizer.tokensPerEncodedByte) := by
  unfold ProjectedPromptTokens
  calc
    promptFixedTokens + tokenizer.count encodedBytes
        ≤ promptFixedTokens
          + (tokenizer.overhead
            + encodedBytes * tokenizer.tokensPerEncodedByte) :=
      Nat.add_le_add_left (tokenizer.bound encodedBytes) promptFixedTokens
    _ ≤ promptFixedTokens
          + (tokenizer.overhead
            + encodedByteCapacity * tokenizer.tokensPerEncodedByte) := by
      exact Nat.add_le_add_left
        (Nat.add_le_add_left
          (Nat.mul_le_mul_right
            tokenizer.tokensPerEncodedByte
            encodedBound)
          tokenizer.overhead)
        promptFixedTokens

theorem recovery_projection_bytes_are_attempt_and_capacity_bounded
    (attempts maxAttempts fixedBytes candidateCount candidateCapacity
      candidateReceiptBytes comparisonCount comparisonReceiptBytes : Nat)
    (attemptBound : attempts ≤ maxAttempts)
    (candidateBound : candidateCount ≤ candidateCapacity)
    (comparisonBound : comparisonCount ≤ candidateCount) :
    attempts
        * EncodedProjectionBytes
          fixedBytes
          candidateCount
          candidateReceiptBytes
          comparisonCount
          comparisonReceiptBytes
      ≤
    maxAttempts
        * EncodedProjectionBytes
          fixedBytes
          candidateCapacity
          candidateReceiptBytes
          candidateCapacity
          comparisonReceiptBytes := by
  calc
    attempts
          * EncodedProjectionBytes
            fixedBytes
            candidateCount
            candidateReceiptBytes
            comparisonCount
            comparisonReceiptBytes
        ≤ attempts
          * EncodedProjectionBytes
            fixedBytes
            candidateCapacity
            candidateReceiptBytes
            candidateCapacity
            comparisonReceiptBytes :=
      Nat.mul_le_mul_left attempts
        (projection_bytes_are_bounded_by_candidate_capacity
          fixedBytes
          candidateCount
          candidateCapacity
          candidateReceiptBytes
          comparisonCount
          comparisonReceiptBytes
          candidateBound
          comparisonBound)
    _ ≤ maxAttempts
          * EncodedProjectionBytes
            fixedBytes
            candidateCapacity
            candidateReceiptBytes
            candidateCapacity
            comparisonReceiptBytes :=
      Nat.mul_le_mul_right
        (EncodedProjectionBytes
          fixedBytes
          candidateCapacity
          candidateReceiptBytes
          candidateCapacity
          comparisonReceiptBytes)
        attemptBound

theorem escaped_json_size_exceeds_raw_size
    (rawBytes : Nat) :
    rawBytes < EscapedJsonStringBytes rawBytes := by
  unfold EscapedJsonStringBytes
  exact Nat.lt_of_lt_of_le
    (Nat.lt_succ_self rawBytes)
    (Nat.le_add_right (Nat.succ rawBytes) (5 * rawBytes + 1))

theorem raw_cap_without_encoder_contract_does_not_bound_encoded_bytes
    (rawCapacity encodedCapacity : Nat) :
    ∃ rawBytes encodedBytes,
      rawBytes ≤ rawCapacity ∧ encodedCapacity < encodedBytes := by
  exact ⟨0, encodedCapacity + 1, Nat.zero_le rawCapacity, Nat.lt_succ_self encodedCapacity⟩

theorem unchanged_projection_identity_is_compatible
    (identity : ProjectionIdentity) :
    ProjectionCompatible identity identity := by
  rfl

theorem renderer_change_invalidates_projection_identity
    (graphDigest rendererVersion changedRendererVersion
      projectionPolicyDigest schemaDigest : Nat)
    (changed : rendererVersion ≠ changedRendererVersion) :
    ¬ ProjectionCompatible
      ⟨graphDigest, rendererVersion, projectionPolicyDigest, schemaDigest⟩
      ⟨graphDigest, changedRendererVersion, projectionPolicyDigest, schemaDigest⟩ := by
  intro compatible
  unfold ProjectionCompatible at compatible
  exact changed (congrArg ProjectionIdentity.rendererVersion compatible)

theorem unchanged_search_result_cache_key_is_compatible
    (key : SearchResultCacheKey) :
    SearchResultCacheCompatible key key := by
  rfl

theorem same_rendered_prefix_does_not_validate_changed_search_snapshot
    (_renderedPrefixDigest workspaceSnapshotDigest changedWorkspaceSnapshotDigest
      queryDigest searchPolicyDigest providerSchemaDigest : Nat)
    (changed : workspaceSnapshotDigest ≠ changedWorkspaceSnapshotDigest) :
    ¬ SearchResultCacheCompatible
      ⟨workspaceSnapshotDigest, queryDigest, searchPolicyDigest, providerSchemaDigest⟩
      ⟨changedWorkspaceSnapshotDigest, queryDigest, searchPolicyDigest, providerSchemaDigest⟩ := by
  intro compatible
  unfold SearchResultCacheCompatible at compatible
  exact changed
    (congrArg SearchResultCacheKey.workspaceSnapshotDigest compatible)

theorem unchanged_model_prefix_cache_key_is_compatible
    (key : ModelPrefixCacheKey) :
    ModelPrefixCacheCompatible key key := by
  rfl

theorem same_rendered_prefix_does_not_validate_changed_model
    (modelDigest changedModelDigest systemPromptDigest toolSchemaDigest
      renderedPrefixDigest prefixBoundaryDigest : Nat)
    (changed : modelDigest ≠ changedModelDigest) :
    ¬ ModelPrefixCacheCompatible
      ⟨modelDigest, systemPromptDigest, toolSchemaDigest,
        renderedPrefixDigest, prefixBoundaryDigest⟩
      ⟨changedModelDigest, systemPromptDigest, toolSchemaDigest,
        renderedPrefixDigest, prefixBoundaryDigest⟩ := by
  intro compatible
  unfold ModelPrefixCacheCompatible at compatible
  exact changed (congrArg ModelPrefixCacheKey.modelDigest compatible)

theorem search_result_cache_validity_does_not_imply_model_prefix_cache_validity :
    ∃ searchLeft searchRight modelLeft modelRight,
      SearchResultCacheCompatible searchLeft searchRight
        ∧ ¬ ModelPrefixCacheCompatible modelLeft modelRight := by
  refine ⟨
    ⟨0, 0, 0, 0⟩,
    ⟨0, 0, 0, 0⟩,
    ⟨0, 0, 0, 0, 0⟩,
    ⟨1, 0, 0, 0, 0⟩,
    rfl,
    ?_⟩
  intro compatible
  unfold ModelPrefixCacheCompatible at compatible
  exact Nat.noConfusion
    (congrArg ModelPrefixCacheKey.modelDigest compatible)

theorem model_prefix_cache_validity_does_not_imply_search_result_cache_validity :
    ∃ modelLeft modelRight searchLeft searchRight,
      ModelPrefixCacheCompatible modelLeft modelRight
        ∧ ¬ SearchResultCacheCompatible searchLeft searchRight := by
  refine ⟨
    ⟨0, 0, 0, 0, 0⟩,
    ⟨0, 0, 0, 0, 0⟩,
    ⟨0, 0, 0, 0⟩,
    ⟨1, 0, 0, 0⟩,
    rfl,
    ?_⟩
  intro compatible
  unfold SearchResultCacheCompatible at compatible
  exact Nat.noConfusion
    (congrArg SearchResultCacheKey.workspaceSnapshotDigest compatible)

end ASPProof.SearchRouteAdmissionRetryCacheRejoinProjectionByteTokenCacheBoundary
