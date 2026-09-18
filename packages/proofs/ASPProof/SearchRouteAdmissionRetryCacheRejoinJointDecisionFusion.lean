-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteAdmissionRetryCacheRejoinJointWeightScheduleTransition

namespace ASPProof.SearchRouteAdmissionRetryCacheRejoinJointDecisionFusion

open ASPProof.SearchRouteAdmissionRetryCacheRejoinCanonicalMembership

inductive ConservativeMaximum
    (oldValue newValue : VotingWeight) :
    VotingWeight → Prop
  | oldSelected
      (newLeOld : newValue ≤ oldValue) :
      ConservativeMaximum oldValue newValue oldValue
  | newSelected
      (oldLeNew : oldValue ≤ newValue) :
      ConservativeMaximum oldValue newValue newValue

def BothInputsVerified
    (oldVerified newVerified : Prop) : Prop :=
  oldVerified ∧ newVerified

structure JointDecisionReceiptKey
    (TransitionId ReceiptId PolicyId : Type) where
  transitionId : TransitionId
  oldWeightedReceiptId : ReceiptId
  newWeightedReceiptId : ReceiptId
  decisionPolicyVersion : PolicyId
  fusedValue : VotingWeight

def JointDecisionCompatible
    {TransitionId ReceiptId PolicyId : Type}
    (left right :
      JointDecisionReceiptKey TransitionId ReceiptId PolicyId) : Prop :=
  left.transitionId = right.transitionId ∧
  left.oldWeightedReceiptId =
      right.oldWeightedReceiptId ∧
  left.newWeightedReceiptId =
      right.newWeightedReceiptId ∧
  left.decisionPolicyVersion =
      right.decisionPolicyVersion ∧
  left.fusedValue = right.fusedValue

theorem conservative_maximum_exists
    (oldValue newValue : VotingWeight) :
    ∃ fusedValue,
      ConservativeMaximum oldValue newValue fusedValue := by
  cases Nat.le_total oldValue newValue with
  | inl oldLeNew =>
      exact ⟨newValue,
        ConservativeMaximum.newSelected oldLeNew⟩
  | inr newLeOld =>
      exact ⟨oldValue,
        ConservativeMaximum.oldSelected newLeOld⟩

theorem conservative_maximum_is_safe_and_live
    (actualFaultWeight liveUpperBound : VotingWeight)
    (oldValue newValue fusedValue : VotingWeight)
    (oldBounds :
      actualFaultWeight ≤ oldValue ∧
        oldValue ≤ liveUpperBound)
    (newBounds :
      actualFaultWeight ≤ newValue ∧
        newValue ≤ liveUpperBound)
    (fusion :
      ConservativeMaximum oldValue newValue fusedValue) :
    actualFaultWeight ≤ fusedValue ∧
      fusedValue ≤ liveUpperBound := by
  cases fusion with
  | oldSelected =>
      exact oldBounds
  | newSelected =>
      exact newBounds

theorem conservative_maximum_dominates_both_inputs
    (oldValue newValue fusedValue : VotingWeight)
    (fusion :
      ConservativeMaximum oldValue newValue fusedValue) :
    oldValue ≤ fusedValue ∧ newValue ≤ fusedValue := by
  cases fusion with
  | oldSelected newLeOld =>
      exact ⟨Nat.le_refl oldValue, newLeOld⟩
  | newSelected oldLeNew =>
      exact ⟨oldLeNew, Nat.le_refl newValue⟩

theorem interval_membership_alone_does_not_imply_conservative_dominance :
    ∃ actualFaultWeight liveUpperBound
        oldValue newValue candidate : VotingWeight,
      actualFaultWeight ≤ oldValue ∧
      oldValue ≤ liveUpperBound ∧
      actualFaultWeight ≤ newValue ∧
      newValue ≤ liveUpperBound ∧
      actualFaultWeight ≤ candidate ∧
      candidate ≤ liveUpperBound ∧
      ¬ (oldValue ≤ candidate ∧ newValue ≤ candidate) := by
  refine ⟨1, 2, 1, 2, 1,
    by decide, by decide, by decide, by decide,
    by decide, by decide, ?_⟩
  intro dominates
  exact (Nat.not_succ_le_self 1) dominates.2

theorem missing_new_verification_blocks_joint_decision
    (oldVerified : Prop) :
    ¬ BothInputsVerified oldVerified False := by
  intro bothVerified
  exact bothVerified.2

theorem missing_old_verification_blocks_joint_decision
    (newVerified : Prop) :
    ¬ BothInputsVerified False newVerified := by
  intro bothVerified
  exact bothVerified.1

theorem joint_decision_compatibility_binds_both_inputs
    {TransitionId ReceiptId PolicyId : Type}
    (left right :
      JointDecisionReceiptKey TransitionId ReceiptId PolicyId)
    (compatible : JointDecisionCompatible left right) :
    left.oldWeightedReceiptId =
        right.oldWeightedReceiptId ∧
      left.newWeightedReceiptId =
        right.newWeightedReceiptId :=
  ⟨compatible.2.1, compatible.2.2.1⟩

theorem changed_old_input_invalidates_joint_decision
    {TransitionId ReceiptId PolicyId : Type}
    (left right :
      JointDecisionReceiptKey TransitionId ReceiptId PolicyId)
    (oldInputChanged :
      left.oldWeightedReceiptId ≠
        right.oldWeightedReceiptId) :
    ¬ JointDecisionCompatible left right := by
  intro compatible
  exact oldInputChanged compatible.2.1

theorem changed_new_input_invalidates_joint_decision
    {TransitionId ReceiptId PolicyId : Type}
    (left right :
      JointDecisionReceiptKey TransitionId ReceiptId PolicyId)
    (newInputChanged :
      left.newWeightedReceiptId ≠
        right.newWeightedReceiptId) :
    ¬ JointDecisionCompatible left right := by
  intro compatible
  exact newInputChanged compatible.2.2.1

theorem same_fused_value_does_not_imply_joint_compatibility
    {TransitionId ReceiptId PolicyId : Type}
    (left right :
      JointDecisionReceiptKey TransitionId ReceiptId PolicyId)
    (sameFusedValue :
      left.fusedValue = right.fusedValue)
    (oldInputChanged :
      left.oldWeightedReceiptId ≠
        right.oldWeightedReceiptId) :
    left.fusedValue = right.fusedValue ∧
      ¬ JointDecisionCompatible left right :=
  ⟨sameFusedValue,
    changed_old_input_invalidates_joint_decision
      left right oldInputChanged⟩

theorem lane_swap_with_distinct_receipts_changes_joint_identity
    {TransitionId ReceiptId PolicyId : Type}
    (left right :
      JointDecisionReceiptKey TransitionId ReceiptId PolicyId)
    (laneSwap :
      left.oldWeightedReceiptId =
          right.newWeightedReceiptId ∧
        left.newWeightedReceiptId =
          right.oldWeightedReceiptId)
    (laneReceiptsDiffer :
      left.oldWeightedReceiptId ≠
        left.newWeightedReceiptId) :
    ¬ JointDecisionCompatible left right := by
  intro compatible
  apply laneReceiptsDiffer
  calc
    left.oldWeightedReceiptId =
        right.oldWeightedReceiptId :=
      compatible.2.1
    _ = left.newWeightedReceiptId :=
      laneSwap.2.symm

end ASPProof.SearchRouteAdmissionRetryCacheRejoinJointDecisionFusion
