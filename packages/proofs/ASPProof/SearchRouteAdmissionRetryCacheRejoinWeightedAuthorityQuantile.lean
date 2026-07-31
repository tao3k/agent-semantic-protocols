import ASPProof.SearchRouteAdmissionRetryCacheRejoinGeneralAuthorityOrderStatistic

namespace ASPProof.SearchRouteAdmissionRetryCacheRejoinWeightedAuthorityQuantile

open ASPProof.SearchRouteAdmissionRetryCacheRejoinCanonicalMembership

/--
The collected honest voting weight must be strictly greater than the admitted
Byzantine voting-weight budget.
-/
def WeightedCollectedQuorum
    (totalCollectedWeight faultWeightBudget : VotingWeight) : Prop :=
  faultWeightBudget + 1 + faultWeightBudget ≤ totalCollectedWeight

/--
`cutWeight` is an inclusive cumulative-weight threshold. It must be above the
Byzantine lower tail and leave more than the Byzantine budget above the cut.
-/
def WeightedCutWindow
    (totalCollectedWeight faultWeightBudget cutWeight : VotingWeight) : Prop :=
  faultWeightBudget < cutWeight ∧
  cutWeight + faultWeightBudget ≤ totalCollectedWeight

theorem weighted_quorum_places_canonical_cut_in_window
    (totalCollectedWeight faultWeightBudget : VotingWeight)
    (quorum :
      WeightedCollectedQuorum
        totalCollectedWeight faultWeightBudget) :
    WeightedCutWindow
      totalCollectedWeight
      faultWeightBudget
      (faultWeightBudget + 1) := by
  unfold WeightedCollectedQuorum at quorum
  unfold WeightedCutWindow
  exact ⟨Nat.lt_succ_self faultWeightBudget, quorum⟩

theorem weighted_cut_certificate_is_safe_and_live
    (totalCollectedWeight faultWeightBudget cutWeight : VotingWeight)
    (actualFaultWeight liveUpperBound selected : VotingWeight)
    (belowActualWeight aboveLiveWeight : VotingWeight)
    (window :
      WeightedCutWindow
        totalCollectedWeight faultWeightBudget cutWeight)
    (belowActualBound :
      belowActualWeight ≤ faultWeightBudget)
    (aboveLiveBound :
      aboveLiveWeight ≤ faultWeightBudget)
    (lowerCutLaw :
      selected < actualFaultWeight →
        cutWeight ≤ belowActualWeight)
    (upperCutLaw :
      liveUpperBound < selected →
        totalCollectedWeight <
          cutWeight + aboveLiveWeight) :
    actualFaultWeight ≤ selected ∧ selected ≤ liveUpperBound := by
  constructor
  · apply Nat.le_of_not_gt
    intro selectedBelowActual
    have cutWithinFaultBudget :
        cutWeight ≤ faultWeightBudget :=
      Nat.le_trans
        (lowerCutLaw selectedBelowActual)
        belowActualBound
    exact
      (Nat.not_lt_of_ge cutWithinFaultBudget)
        window.1
  · apply Nat.le_of_not_gt
    intro selectedAboveLive
    have cutAndAboveWithinTotal :
        cutWeight + aboveLiveWeight ≤ totalCollectedWeight :=
      Nat.le_trans
        (Nat.add_le_add_left aboveLiveBound cutWeight)
        window.2
    exact
      (Nat.not_lt_of_ge cutAndAboveWithinTotal)
        (upperCutLaw selectedAboveLive)

theorem weighted_cut_window_implies_weighted_collected_quorum
    (totalCollectedWeight faultWeightBudget cutWeight : VotingWeight)
    (window :
      WeightedCutWindow
        totalCollectedWeight faultWeightBudget cutWeight) :
    WeightedCollectedQuorum
      totalCollectedWeight faultWeightBudget := by
  unfold WeightedCutWindow at window
  unfold WeightedCollectedQuorum
  have canonicalCutLeSelectedCut :
      faultWeightBudget + 1 ≤ cutWeight := by
    rw [Nat.add_one]
    exact Nat.succ_le_of_lt window.1
  exact
    Nat.le_trans
      (Nat.add_le_add_right
        canonicalCutLeSelectedCut
        faultWeightBudget)
      window.2

theorem voting_weight_budget_does_not_imply_authority_count_budget :
    ∃ totalCollectedWeight faultWeightBudget
        totalAuthorityCount assumedFaultCount
        byzantineAuthorityCount byzantineAggregateWeight : Nat,
      WeightedCollectedQuorum
        totalCollectedWeight faultWeightBudget ∧
      2 * assumedFaultCount + 1 ≤ totalAuthorityCount ∧
      assumedFaultCount < byzantineAuthorityCount ∧
      byzantineAggregateWeight ≤ faultWeightBudget := by
  exact ⟨10, 3, 5, 2, 3, 3,
    by unfold WeightedCollectedQuorum; decide,
    by decide,
    by decide,
    by decide⟩

theorem count_selected_value_can_violate_weighted_lower_bound :
    ∃ totalCollectedWeight faultWeightBudget
        actualFaultWeight liveUpperBound selected : VotingWeight,
      WeightedCollectedQuorum
        totalCollectedWeight faultWeightBudget ∧
      actualFaultWeight ≤ liveUpperBound ∧
      selected < actualFaultWeight := by
  exact ⟨10, 3, 3, 3, 0,
    by unfold WeightedCollectedQuorum; decide,
    by decide,
    by decide⟩

theorem unweighted_report_count_does_not_establish_weighted_quorum :
    ∃ reportCount assumedFaultCount
        totalCollectedWeight faultWeightBudget : Nat,
      2 * assumedFaultCount + 1 ≤ reportCount ∧
      ¬ WeightedCollectedQuorum
          totalCollectedWeight faultWeightBudget := by
  refine ⟨3, 1, 10, 8, by decide, ?_⟩
  intro weightedQuorum
  unfold WeightedCollectedQuorum at weightedQuorum
  have impossible : 17 ≤ 16 :=
    Nat.le_trans weightedQuorum (by decide)
  exact (Nat.not_succ_le_self 16) impossible

end ASPProof.SearchRouteAdmissionRetryCacheRejoinWeightedAuthorityQuantile
