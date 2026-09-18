-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteAdmissionRetryCacheRejoinResilientAuthorityMedian

namespace ASPProof.SearchRouteAdmissionRetryCacheRejoinGeneralAuthorityOrderStatistic

open ASPProof.SearchRouteAdmissionRetryCacheRejoinCanonicalMembership

/--
For a zero-indexed order statistic at `rank`, both tails must contain more
distinct admitted reports than the Byzantine fault budget.
-/
def ZeroIndexedRankWindow
    (reportCount faultBudget rank : Nat) : Prop :=
  faultBudget ≤ rank ∧
  rank + faultBudget < reportCount

theorem quorum_places_fault_rank_interior
    (reportCount faultBudget : Nat)
    (quorum : 2 * faultBudget + 1 ≤ reportCount) :
    ZeroIndexedRankWindow reportCount faultBudget faultBudget := by
  unfold ZeroIndexedRankWindow
  constructor
  · exact Nat.le_refl faultBudget
  · have doubledFaultBudgetLt :
        2 * faultBudget < reportCount :=
      Nat.lt_of_succ_le (by
        rw [Nat.add_one] at quorum
        exact quorum)
    rw [Nat.two_mul] at doubledFaultBudgetLt
    exact doubledFaultBudgetLt

/--
The proof does not trust an implementation-specific sorting algorithm. It
trusts a rank certificate whose lower and upper witness counts are checked
against the same distinct, admitted report set.
-/
theorem rank_certified_order_statistic_is_safe_and_live
    (reportCount faultBudget rank : Nat)
    (actualFaultWeight liveUpperBound selected : VotingWeight)
    (belowActualCount aboveLiveCount : Nat)
    (window :
      ZeroIndexedRankWindow reportCount faultBudget rank)
    (belowActualBound : belowActualCount ≤ faultBudget)
    (aboveLiveBound : aboveLiveCount ≤ faultBudget)
    (lowerRankLaw :
      selected < actualFaultWeight →
        rank + 1 ≤ belowActualCount)
    (upperRankLaw :
      liveUpperBound < selected →
        reportCount ≤ rank + aboveLiveCount) :
    actualFaultWeight ≤ selected ∧ selected ≤ liveUpperBound := by
  constructor
  · apply Nat.le_of_not_gt
    intro selectedBelowActual
    have lowerWitnessWithinFaultBudget :
        rank + 1 ≤ faultBudget :=
      Nat.le_trans
        (lowerRankLaw selectedBelowActual)
        belowActualBound
    have faultBudgetSuccessorWithinFaultBudget :
        faultBudget + 1 ≤ faultBudget :=
      Nat.le_trans
        (Nat.add_le_add_right window.1 1)
        lowerWitnessWithinFaultBudget
    exact
      (Nat.not_succ_le_self faultBudget)
        (by
          rw [Nat.add_one] at faultBudgetSuccessorWithinFaultBudget
          exact faultBudgetSuccessorWithinFaultBudget)
  · apply Nat.le_of_not_gt
    intro selectedAboveLive
    have reportCountWithinRankAndFaultBudget :
        reportCount ≤ rank + faultBudget :=
      Nat.le_trans
        (upperRankLaw selectedAboveLive)
        (Nat.add_le_add_left aboveLiveBound rank)
    exact
      (Nat.not_le_of_lt window.2)
        reportCountWithinRankAndFaultBudget

theorem rank_window_implies_collected_quorum
    (reportCount faultBudget rank : Nat)
    (window :
      ZeroIndexedRankWindow reportCount faultBudget rank) :
    2 * faultBudget + 1 ≤ reportCount := by
  unfold ZeroIndexedRankWindow at window
  have doubledFaultBudgetLt :
      faultBudget + faultBudget < reportCount :=
    Nat.lt_of_le_of_lt
      (Nat.add_le_add_right window.1 faultBudget)
      window.2
  have doubledFaultBudgetSuccessorLe :
      Nat.succ (faultBudget + faultBudget) ≤ reportCount :=
    Nat.succ_le_of_lt doubledFaultBudgetLt
  rw [Nat.two_mul, Nat.add_one]
  exact doubledFaultBudgetSuccessorLe

theorem configured_quorum_does_not_cover_partial_collection :
    ∃ configuredAuthorityCount collectedReportCount faultBudget rank : Nat,
      2 * faultBudget + 1 ≤ configuredAuthorityCount ∧
      collectedReportCount < configuredAuthorityCount ∧
      ¬ ZeroIndexedRankWindow
          collectedReportCount faultBudget rank := by
  refine ⟨5, 3, 2, 1, by decide, by decide, ?_⟩
  intro window
  exact (Nat.not_succ_le_self 1) window.1

theorem rankless_selector_can_violate_actual_fault_lower_bound :
    ∃ reportCount faultBudget : Nat,
      ∃ actualFaultWeight liveUpperBound selected : VotingWeight,
        2 * faultBudget + 1 ≤ reportCount ∧
        actualFaultWeight ≤ liveUpperBound ∧
        selected < actualFaultWeight := by
  exact ⟨3, 1, 1, 1, 0, by decide, by decide, by decide⟩

theorem rankless_selector_can_violate_live_upper_bound :
    ∃ reportCount faultBudget : Nat,
      ∃ actualFaultWeight liveUpperBound selected : VotingWeight,
        2 * faultBudget + 1 ≤ reportCount ∧
        actualFaultWeight ≤ liveUpperBound ∧
        liveUpperBound < selected := by
  exact ⟨3, 1, 1, 1, 2, by decide, by decide, by decide⟩

end ASPProof.SearchRouteAdmissionRetryCacheRejoinGeneralAuthorityOrderStatistic
