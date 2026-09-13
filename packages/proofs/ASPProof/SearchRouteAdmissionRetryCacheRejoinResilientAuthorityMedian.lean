-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteAdmissionRetryCacheRejoinMultiAuthorityFaultAggregation

namespace ASPProof.SearchRouteAdmissionRetryCacheRejoinResilientAuthorityMedian

open ASPProof.SearchRouteAdmissionRetryCacheRejoinCanonicalMembership
open ASPProof.SearchRouteAdmissionRetryCacheRejoinMultiAuthorityFaultAggregation

def HonestBound
    (actualFaultWeight liveUpperBound report : VotingWeight) : Prop :=
  actualFaultWeight ≤ report ∧ report ≤ liveUpperBound

def AtLeastTwoHonestBounds
    (actualFaultWeight liveUpperBound : VotingWeight)
    (first second third : VotingWeight) : Prop :=
  (HonestBound actualFaultWeight liveUpperBound first ∧
    HonestBound actualFaultWeight liveUpperBound second) ∨
  (HonestBound actualFaultWeight liveUpperBound first ∧
    HonestBound actualFaultWeight liveUpperBound third) ∨
  (HonestBound actualFaultWeight liveUpperBound second ∧
    HonestBound actualFaultWeight liveUpperBound third)

def PairwiseDistinct
    {AuthorityId : Type}
    (first second third : AuthorityId) : Prop :=
  first ≠ second ∧ first ≠ third ∧ second ≠ third

inductive MedianOfThree
    (first second third : VotingWeight) :
    VotingWeight → Prop
  | firstSecondThird
      (firstLeSecond : first ≤ second)
      (secondLeThird : second ≤ third) :
      MedianOfThree first second third second
  | firstThirdSecond
      (firstLeThird : first ≤ third)
      (thirdLeSecond : third ≤ second) :
      MedianOfThree first second third third
  | secondFirstThird
      (secondLeFirst : second ≤ first)
      (firstLeThird : first ≤ third) :
      MedianOfThree first second third first
  | secondThirdFirst
      (secondLeThird : second ≤ third)
      (thirdLeFirst : third ≤ first) :
      MedianOfThree first second third third
  | thirdFirstSecond
      (thirdLeFirst : third ≤ first)
      (firstLeSecond : first ≤ second) :
      MedianOfThree first second third first
  | thirdSecondFirst
      (thirdLeSecond : third ≤ second)
      (secondLeFirst : second ≤ first) :
      MedianOfThree first second third second

theorem median_order_exists
    (first second third : VotingWeight) :
    ∃ median, MedianOfThree first second third median := by
  cases Nat.le_total first second with
  | inl firstLeSecond =>
      cases Nat.le_total second third with
      | inl secondLeThird =>
          exact ⟨second,
            MedianOfThree.firstSecondThird
              firstLeSecond secondLeThird⟩
      | inr thirdLeSecond =>
          cases Nat.le_total first third with
          | inl firstLeThird =>
              exact ⟨third,
                MedianOfThree.firstThirdSecond
                  firstLeThird thirdLeSecond⟩
          | inr thirdLeFirst =>
              exact ⟨first,
                MedianOfThree.thirdFirstSecond
                  thirdLeFirst firstLeSecond⟩
  | inr secondLeFirst =>
      cases Nat.le_total first third with
      | inl firstLeThird =>
          exact ⟨first,
            MedianOfThree.secondFirstThird
              secondLeFirst firstLeThird⟩
      | inr thirdLeFirst =>
          cases Nat.le_total second third with
          | inl secondLeThird =>
              exact ⟨third,
                MedianOfThree.secondThirdFirst
                  secondLeThird thirdLeFirst⟩
          | inr thirdLeSecond =>
              exact ⟨second,
                MedianOfThree.thirdSecondFirst
                  thirdLeSecond secondLeFirst⟩

theorem resilient_median_is_safe_and_live
    (actualFaultWeight liveUpperBound : VotingWeight)
    (first second third median : VotingWeight)
    (ordering : MedianOfThree first second third median)
    (coverage :
      AtLeastTwoHonestBounds
        actualFaultWeight liveUpperBound first second third) :
    actualFaultWeight ≤ median ∧ median ≤ liveUpperBound := by
  rcases coverage with firstSecond | firstThird | secondThird
  · rcases firstSecond with
      ⟨⟨firstLower, firstUpper⟩,
        ⟨secondLower, secondUpper⟩⟩
    cases ordering with
    | firstSecondThird =>
        exact ⟨secondLower, secondUpper⟩
    | firstThirdSecond firstLeThird thirdLeSecond =>
        exact ⟨
          Nat.le_trans firstLower firstLeThird,
          Nat.le_trans thirdLeSecond secondUpper
        ⟩
    | secondFirstThird =>
        exact ⟨firstLower, firstUpper⟩
    | secondThirdFirst secondLeThird thirdLeFirst =>
        exact ⟨
          Nat.le_trans secondLower secondLeThird,
          Nat.le_trans thirdLeFirst firstUpper
        ⟩
    | thirdFirstSecond =>
        exact ⟨firstLower, firstUpper⟩
    | thirdSecondFirst =>
        exact ⟨secondLower, secondUpper⟩
  · rcases firstThird with
      ⟨⟨firstLower, firstUpper⟩,
        ⟨thirdLower, thirdUpper⟩⟩
    cases ordering with
    | firstSecondThird firstLeSecond secondLeThird =>
        exact ⟨
          Nat.le_trans firstLower firstLeSecond,
          Nat.le_trans secondLeThird thirdUpper
        ⟩
    | firstThirdSecond =>
        exact ⟨thirdLower, thirdUpper⟩
    | secondFirstThird =>
        exact ⟨firstLower, firstUpper⟩
    | secondThirdFirst =>
        exact ⟨thirdLower, thirdUpper⟩
    | thirdFirstSecond =>
        exact ⟨firstLower, firstUpper⟩
    | thirdSecondFirst thirdLeSecond secondLeFirst =>
        exact ⟨
          Nat.le_trans thirdLower thirdLeSecond,
          Nat.le_trans secondLeFirst firstUpper
        ⟩
  · rcases secondThird with
      ⟨⟨secondLower, secondUpper⟩,
        ⟨thirdLower, thirdUpper⟩⟩
    cases ordering with
    | firstSecondThird =>
        exact ⟨secondLower, secondUpper⟩
    | firstThirdSecond =>
        exact ⟨thirdLower, thirdUpper⟩
    | secondFirstThird secondLeFirst firstLeThird =>
        exact ⟨
          Nat.le_trans secondLower secondLeFirst,
          Nat.le_trans firstLeThird thirdUpper
        ⟩
    | secondThirdFirst =>
        exact ⟨thirdLower, thirdUpper⟩
    | thirdFirstSecond thirdLeFirst firstLeSecond =>
        exact ⟨
          Nat.le_trans thirdLower thirdLeFirst,
          Nat.le_trans firstLeSecond secondUpper
        ⟩
    | thirdSecondFirst =>
        exact ⟨secondLower, secondUpper⟩

def safeMinimum : VotingWeight → VotingWeight → VotingWeight
  | 0, _ => 0
  | _, 0 => 0
  | Nat.succ left, Nat.succ right =>
      Nat.succ (safeMinimum left right)

def maximumOfThree
    (first second third : VotingWeight) : VotingWeight :=
  safeMaximum first (safeMaximum second third)

def minimumOfThree
    (first second third : VotingWeight) : VotingWeight :=
  safeMinimum first (safeMinimum second third)

theorem maximum_can_violate_live_upper_bound :
    ∃ actualFaultWeight liveUpperBound first second byzantine : VotingWeight,
      HonestBound actualFaultWeight liveUpperBound first ∧
      HonestBound actualFaultWeight liveUpperBound second ∧
      liveUpperBound < maximumOfThree first second byzantine := by
  exact ⟨
    1,
    1,
    1,
    1,
    2,
    ⟨Nat.le_refl 1, Nat.le_refl 1⟩,
    ⟨Nat.le_refl 1, Nat.le_refl 1⟩,
    Nat.lt_succ_self 1
  ⟩

theorem minimum_can_violate_actual_fault_lower_bound :
    ∃ actualFaultWeight liveUpperBound first second byzantine : VotingWeight,
      HonestBound actualFaultWeight liveUpperBound first ∧
      HonestBound actualFaultWeight liveUpperBound second ∧
      minimumOfThree first second byzantine < actualFaultWeight := by
  exact ⟨
    1,
    1,
    1,
    1,
    0,
    ⟨Nat.le_refl 1, Nat.le_refl 1⟩,
    ⟨Nat.le_refl 1, Nat.le_refl 1⟩,
    Nat.zero_lt_succ 0
  ⟩

theorem equivocation_cannot_fill_two_distinct_authority_slots
    {AuthorityId : Type}
    (authority otherAuthority : AuthorityId) :
    ¬ PairwiseDistinct authority authority otherAuthority := by
  intro distinct
  exact distinct.1 rfl

end ASPProof.SearchRouteAdmissionRetryCacheRejoinResilientAuthorityMedian
