-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteAdmissionRetryCacheRejoinProjectionByteTokenCacheBoundary

namespace ASPProof.SearchRouteCanonicalReceiptGrammarProgressiveInspectBound

def SeparatorCount : Nat → Nat
  | 0 => 0
  | Nat.succ count => count

def JsonArrayBytes
    (itemCount totalEncodedItemBytes : Nat) : Nat :=
  2 + totalEncodedItemBytes + SeparatorCount itemCount

def OverflowSummaryBytes
    (encodedCountBytes fixedSummaryBytes digestBytes : Nat) : Nat :=
  encodedCountBytes + (fixedSummaryBytes + digestBytes)

def CanonicalProjectionBytes
    (fixedObjectBytes candidateCount totalCandidateBytes
      comparisonCount totalComparisonBytes overflowSummaryBytes : Nat) : Nat :=
  fixedObjectBytes
    + JsonArrayBytes candidateCount totalCandidateBytes
    + JsonArrayBytes comparisonCount totalComparisonBytes
    + overflowSummaryBytes

def CanonicalProjectionCapacityBytes
    (fixedObjectBytes candidateCapacity candidateReceiptByteMaximum
      comparisonReceiptByteMaximum overflowSummaryByteMaximum : Nat) : Nat :=
  fixedObjectBytes
    + (2 + candidateCapacity * candidateReceiptByteMaximum + candidateCapacity)
    + (2 + candidateCapacity * comparisonReceiptByteMaximum + candidateCapacity)
    + overflowSummaryByteMaximum

structure InspectBudget where
  maxSteps : Nat
  maxEncodedBytes : Nat

structure InspectBudgetState where
  stepsUsed : Nat
  encodedBytesUsed : Nat

def InspectAuthorized
    (budget : InspectBudget)
    (state : InspectBudgetState)
    (nextStepEncodedBytes : Nat) : Prop :=
  state.stepsUsed < budget.maxSteps
    ∧ state.encodedBytesUsed + nextStepEncodedBytes
      ≤ budget.maxEncodedBytes

def applyInspect
    (state : InspectBudgetState)
    (nextStepEncodedBytes : Nat) : InspectBudgetState :=
  {
    stepsUsed := Nat.succ state.stepsUsed
    encodedBytesUsed := state.encodedBytesUsed + nextStepEncodedBytes
  }

def InspectSequenceBytes
    (initialProjectionBytes : Nat)
    (stepBytes : List Nat) : Nat :=
  initialProjectionBytes + stepBytes.sum

structure InspectIdentity where
  workspaceSnapshotDigest : Nat
  projectionIdentityDigest : Nat
  selectorDigest : Nat
  depth : Nat
  expansionPolicyDigest : Nat

def InspectCompatible
    (left right : InspectIdentity) : Prop :=
  left = right

theorem separator_count_zero :
    SeparatorCount 0 = 0 := by
  rfl

theorem separator_count_succ
    (count : Nat) :
    SeparatorCount (Nat.succ count) = count := by
  rfl

theorem separator_count_is_bounded_by_item_count
    (count : Nat) :
    SeparatorCount count ≤ count := by
  cases count with
  | zero =>
      exact Nat.le_refl 0
  | succ predecessor =>
      exact Nat.le_succ predecessor

theorem empty_json_array_has_two_bytes :
    JsonArrayBytes 0 0 = 2 := by
  rfl

theorem json_array_bytes_are_capacity_bounded
    (itemCount itemCapacity totalEncodedItemBytes
      encodedItemByteMaximum : Nat)
    (itemCountBound : itemCount ≤ itemCapacity)
    (itemBytesBound :
      totalEncodedItemBytes ≤ itemCount * encodedItemByteMaximum) :
    JsonArrayBytes itemCount totalEncodedItemBytes
      ≤ 2
        + itemCapacity * encodedItemByteMaximum
        + itemCapacity := by
  have payloadCapacityBound :
      totalEncodedItemBytes
        ≤ itemCapacity * encodedItemByteMaximum :=
    Nat.le_trans
      itemBytesBound
      (Nat.mul_le_mul_right encodedItemByteMaximum itemCountBound)
  have separatorCapacityBound :
      SeparatorCount itemCount ≤ itemCapacity :=
    Nat.le_trans
      (separator_count_is_bounded_by_item_count itemCount)
      itemCountBound
  unfold JsonArrayBytes
  exact Nat.add_le_add
    (Nat.add_le_add_left payloadCapacityBound 2)
    separatorCapacityBound

theorem item_count_cap_without_item_byte_cap_does_not_bound_payload
    (encodedByteCapacity : Nat) :
    ∃ oneItemEncodedBytes,
      encodedByteCapacity < oneItemEncodedBytes := by
  exact ⟨Nat.succ encodedByteCapacity, Nat.lt_succ_self encodedByteCapacity⟩

theorem overflow_summary_is_bounded_by_encoded_count_cap
    (encodedCountBytes encodedCountByteMaximum
      fixedSummaryBytes digestBytes : Nat)
    (countBytesBound : encodedCountBytes ≤ encodedCountByteMaximum) :
    OverflowSummaryBytes
        encodedCountBytes
        fixedSummaryBytes
        digestBytes
      ≤
    OverflowSummaryBytes
        encodedCountByteMaximum
        fixedSummaryBytes
        digestBytes := by
  unfold OverflowSummaryBytes
  exact Nat.add_le_add_right
    countBytesBound
    (fixedSummaryBytes + digestBytes)

theorem fixed_digest_without_count_byte_cap_does_not_bound_overflow_summary
    (fixedSummaryBytes digestBytes byteCapacity : Nat) :
    ∃ encodedCountBytes,
      byteCapacity
        < OverflowSummaryBytes
          encodedCountBytes
          fixedSummaryBytes
          digestBytes := by
  refine ⟨Nat.succ byteCapacity, ?_⟩
  unfold OverflowSummaryBytes
  exact Nat.lt_of_lt_of_le
    (Nat.lt_succ_self byteCapacity)
    (Nat.le_add_right
      (Nat.succ byteCapacity)
      (fixedSummaryBytes + digestBytes))

theorem canonical_projection_bytes_are_capacity_bounded
    (fixedObjectBytes candidateCount candidateCapacity
      totalCandidateBytes candidateReceiptByteMaximum
      comparisonCount totalComparisonBytes comparisonReceiptByteMaximum
      overflowSummaryBytes overflowSummaryByteMaximum : Nat)
    (candidateCountBound : candidateCount ≤ candidateCapacity)
    (candidateBytesBound :
      totalCandidateBytes
        ≤ candidateCount * candidateReceiptByteMaximum)
    (comparisonCountBound : comparisonCount ≤ candidateCount)
    (comparisonBytesBound :
      totalComparisonBytes
        ≤ comparisonCount * comparisonReceiptByteMaximum)
    (overflowBound :
      overflowSummaryBytes ≤ overflowSummaryByteMaximum) :
    CanonicalProjectionBytes
        fixedObjectBytes
        candidateCount
        totalCandidateBytes
        comparisonCount
        totalComparisonBytes
        overflowSummaryBytes
      ≤
    CanonicalProjectionCapacityBytes
        fixedObjectBytes
        candidateCapacity
        candidateReceiptByteMaximum
        comparisonReceiptByteMaximum
        overflowSummaryByteMaximum := by
  have candidateArrayBound :=
    json_array_bytes_are_capacity_bounded
      candidateCount
      candidateCapacity
      totalCandidateBytes
      candidateReceiptByteMaximum
      candidateCountBound
      candidateBytesBound
  have comparisonCapacityBound :
      comparisonCount ≤ candidateCapacity :=
    Nat.le_trans comparisonCountBound candidateCountBound
  have comparisonArrayBound :=
    json_array_bytes_are_capacity_bounded
      comparisonCount
      candidateCapacity
      totalComparisonBytes
      comparisonReceiptByteMaximum
      comparisonCapacityBound
      comparisonBytesBound
  unfold CanonicalProjectionBytes CanonicalProjectionCapacityBytes
  exact Nat.add_le_add
    (Nat.add_le_add
      (Nat.add_le_add_left candidateArrayBound fixedObjectBytes)
      comparisonArrayBound)
    overflowBound

theorem authorized_inspect_preserves_step_budget
    (budget : InspectBudget)
    (state : InspectBudgetState)
    (nextStepEncodedBytes : Nat)
    (authorized :
      InspectAuthorized budget state nextStepEncodedBytes) :
    (applyInspect state nextStepEncodedBytes).stepsUsed
      ≤ budget.maxSteps := by
  exact Nat.succ_le_of_lt authorized.1

theorem authorized_inspect_preserves_byte_budget
    (budget : InspectBudget)
    (state : InspectBudgetState)
    (nextStepEncodedBytes : Nat)
    (authorized :
      InspectAuthorized budget state nextStepEncodedBytes) :
    (applyInspect state nextStepEncodedBytes).encodedBytesUsed
      ≤ budget.maxEncodedBytes := by
  exact authorized.2

theorem exhausted_step_budget_blocks_inspect
    (maxSteps maxEncodedBytes encodedBytesUsed nextStepEncodedBytes : Nat) :
    ¬ InspectAuthorized
      ⟨maxSteps, maxEncodedBytes⟩
      ⟨maxSteps, encodedBytesUsed⟩
      nextStepEncodedBytes := by
  intro authorized
  exact Nat.lt_irrefl maxSteps authorized.1

theorem inspect_step_sum_is_bounded_by_length
    (stepBytes : List Nat)
    (stepByteMaximum : Nat)
    (eachStepBound :
      ∀ stepBytesValue ∈ stepBytes,
        stepBytesValue ≤ stepByteMaximum) :
    stepBytes.sum ≤ stepBytes.length * stepByteMaximum := by
  induction stepBytes with
  | nil =>
      change 0 ≤ 0 * stepByteMaximum
      rw [Nat.zero_mul]
      exact Nat.le_refl 0
  | cons head tail inductionHypothesis =>
      have headBound : head ≤ stepByteMaximum :=
        eachStepBound head (List.mem_cons_self)
      have tailBound :
          ∀ stepBytesValue ∈ tail,
            stepBytesValue ≤ stepByteMaximum :=
        fun stepBytesValue member =>
          eachStepBound
            stepBytesValue
            (List.mem_cons_of_mem head member)
      have tailSumBound :=
        inductionHypothesis tailBound
      calc
        (head :: tail).sum = head + tail.sum := rfl
        _ ≤ stepByteMaximum + tail.length * stepByteMaximum :=
          Nat.add_le_add headBound tailSumBound
        _ = tail.length * stepByteMaximum + stepByteMaximum :=
          Nat.add_comm
            stepByteMaximum
            (tail.length * stepByteMaximum)
        _ = (head :: tail).length * stepByteMaximum := by
          rw [List.length_cons, Nat.succ_mul]

theorem inspect_sequence_bytes_are_globally_bounded
    (initialProjectionBytes : Nat)
    (stepBytes : List Nat)
    (maxSteps stepByteMaximum : Nat)
    (stepCountBound : stepBytes.length ≤ maxSteps)
    (eachStepBound :
      ∀ stepBytesValue ∈ stepBytes,
        stepBytesValue ≤ stepByteMaximum) :
    InspectSequenceBytes initialProjectionBytes stepBytes
      ≤ initialProjectionBytes + maxSteps * stepByteMaximum := by
  unfold InspectSequenceBytes
  apply Nat.add_le_add_left
  exact Nat.le_trans
    (inspect_step_sum_is_bounded_by_length
      stepBytes
      stepByteMaximum
      eachStepBound)
    (Nat.mul_le_mul_right stepByteMaximum stepCountBound)

theorem per_step_cap_without_interaction_cap_does_not_bound_total
    (byteBudget : Nat) :
    ∃ stepCount,
      byteBudget < stepCount * 1 := by
  refine ⟨Nat.succ byteBudget, ?_⟩
  rw [Nat.mul_one]
  exact Nat.lt_succ_self byteBudget

theorem unchanged_inspect_identity_is_compatible
    (identity : InspectIdentity) :
    InspectCompatible identity identity := by
  rfl

theorem changed_inspect_depth_invalidates_identity
    (workspaceSnapshotDigest projectionIdentityDigest selectorDigest
      depth changedDepth expansionPolicyDigest : Nat)
    (changed : depth ≠ changedDepth) :
    ¬ InspectCompatible
      ⟨workspaceSnapshotDigest, projectionIdentityDigest, selectorDigest,
        depth, expansionPolicyDigest⟩
      ⟨workspaceSnapshotDigest, projectionIdentityDigest, selectorDigest,
        changedDepth, expansionPolicyDigest⟩ := by
  intro compatible
  unfold InspectCompatible at compatible
  exact changed (congrArg InspectIdentity.depth compatible)

theorem changed_inspect_snapshot_invalidates_identity
    (workspaceSnapshotDigest changedWorkspaceSnapshotDigest
      projectionIdentityDigest selectorDigest depth expansionPolicyDigest : Nat)
    (changed :
      workspaceSnapshotDigest ≠ changedWorkspaceSnapshotDigest) :
    ¬ InspectCompatible
      ⟨workspaceSnapshotDigest, projectionIdentityDigest, selectorDigest,
        depth, expansionPolicyDigest⟩
      ⟨changedWorkspaceSnapshotDigest, projectionIdentityDigest,
        selectorDigest, depth, expansionPolicyDigest⟩ := by
  intro compatible
  unfold InspectCompatible at compatible
  exact changed
    (congrArg InspectIdentity.workspaceSnapshotDigest compatible)

end ASPProof.SearchRouteCanonicalReceiptGrammarProgressiveInspectBound
