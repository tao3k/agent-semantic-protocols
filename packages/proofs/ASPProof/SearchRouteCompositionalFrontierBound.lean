-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.SearchRouteCompositionalFrontierBound

def LowerBound (lower actual : Nat) : Prop :=
  lower ≤ actual

theorem and_unknown_sharing_max_is_sound
    (leftLower rightLower leftCost rightCost combinedCost : Nat)
    (leftBound : LowerBound leftLower leftCost)
    (rightBound : LowerBound rightLower rightCost)
    (containsLeft : leftCost ≤ combinedCost)
    (containsRight : rightCost ≤ combinedCost) :
    LowerBound (max leftLower rightLower) combinedCost := by
  exact (Nat.max_le).2
    ⟨
      Nat.le_trans leftBound containsLeft,
      Nat.le_trans rightBound containsRight
    ⟩

theorem and_disjoint_addition_is_sound
    (leftLower rightLower leftCost rightCost combinedCost : Nat)
    (leftBound : LowerBound leftLower leftCost)
    (rightBound : LowerBound rightLower rightCost)
    (additive : combinedCost = leftCost + rightCost) :
    LowerBound (leftLower + rightLower) combinedCost := by
  rw [additive]
  exact Nat.add_le_add leftBound rightBound

structure IdentityAwareAndCost where
  shared : Nat
  leftExclusive : Nat
  rightExclusive : Nat
  deriving DecidableEq, Repr

def IdentityAwareAndCost.combined
    (cost : IdentityAwareAndCost) : Nat :=
  cost.shared + cost.leftExclusive + cost.rightExclusive

theorem identity_aware_and_bound_is_sound
    (sharedLower leftExclusiveLower rightExclusiveLower : Nat)
    (cost : IdentityAwareAndCost)
    (sharedBound : LowerBound sharedLower cost.shared)
    (leftBound :
      LowerBound leftExclusiveLower cost.leftExclusive)
    (rightBound :
      LowerBound rightExclusiveLower cost.rightExclusive) :
    LowerBound
      (sharedLower + leftExclusiveLower + rightExclusiveLower)
      cost.combined := by
  exact Nat.add_le_add
    (Nat.add_le_add sharedBound leftBound)
    rightBound

inductive OrSelection where
  | left
  | right
  deriving DecidableEq, Repr

def selectedCost
    (selection : OrSelection)
    (leftCost rightCost : Nat) : Nat :=
  match selection with
  | .left => leftCost
  | .right => rightCost

theorem or_minimum_bound_is_sound
    (leftLower rightLower leftCost rightCost : Nat)
    (leftBound : LowerBound leftLower leftCost)
    (rightBound : LowerBound rightLower rightCost)
    (selection : OrSelection) :
    LowerBound
      (min leftLower rightLower)
      (selectedCost selection leftCost rightCost) := by
  cases selection with
  | left =>
      exact Nat.le_trans (Nat.min_le_left _ _) leftBound
  | right =>
      exact Nat.le_trans (Nat.min_le_right _ _) rightBound

def sharedWitnessCost : IdentityAwareAndCost :=
  {
    shared := 10
    leftExclusive := 0
    rightExclusive := 0
  }

theorem naive_addition_is_not_a_lower_bound_for_shared_witness :
    ¬ LowerBound (10 + 10) sharedWitnessCost.combined := by
  unfold LowerBound IdentityAwareAndCost.combined sharedWitnessCost
  decide

theorem shared_witness_unknown_sharing_bound_is_exact :
    max 10 10 = sharedWitnessCost.combined := by
  decide

theorem shared_witness_identity_aware_bound_is_exact :
    10 + 0 + 0 = sharedWitnessCost.combined := by
  decide

theorem shared_witness_counterexample :
    ¬ LowerBound (10 + 10) sharedWitnessCost.combined ∧
      LowerBound (max 10 10) sharedWitnessCost.combined ∧
      LowerBound (10 + 0 + 0) sharedWitnessCost.combined := by
  exact
    ⟨
      naive_addition_is_not_a_lower_bound_for_shared_witness,
      by
        unfold LowerBound IdentityAwareAndCost.combined sharedWitnessCost
        decide,
      by
        unfold LowerBound IdentityAwareAndCost.combined sharedWitnessCost
        decide
    ⟩

end ASPProof.SearchRouteCompositionalFrontierBound
