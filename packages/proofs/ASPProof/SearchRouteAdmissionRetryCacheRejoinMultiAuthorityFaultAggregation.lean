import ASPProof.SearchRouteAdmissionRetryCacheRejoinFaultBoundAuthority

namespace ASPProof.SearchRouteAdmissionRetryCacheRejoinMultiAuthorityFaultAggregation

open ASPProof.SearchRouteAdmissionRetryCacheRejoinCanonicalMembership

structure AuthorityBoundCandidate (SnapshotId : Type) where
  snapshotId : SnapshotId
  declaredFaultBound : VotingWeight

def safeMaximum : VotingWeight → VotingWeight → VotingWeight
  | 0, right => right
  | Nat.succ left, 0 => Nat.succ left
  | Nat.succ left, Nat.succ right =>
      Nat.succ (safeMaximum left right)

theorem left_is_at_most_maximum
    (left right : VotingWeight) :
    left ≤ safeMaximum left right := by
  induction left generalizing right with
  | zero => exact Nat.zero_le right
  | succ left inductionHypothesis =>
      cases right with
      | zero => exact Nat.le_refl (Nat.succ left)
      | succ right =>
          exact Nat.succ_le_succ
            (inductionHypothesis right)

theorem right_is_at_most_maximum
    (left right : VotingWeight) :
    right ≤ safeMaximum left right := by
  induction left generalizing right with
  | zero => exact Nat.le_refl right
  | succ left inductionHypothesis =>
      cases right with
      | zero => exact Nat.zero_le (Nat.succ left)
      | succ right =>
          exact Nat.succ_le_succ
            (inductionHypothesis right)

def maximumCandidateBound
    {SnapshotId : Type}
    (candidates : List (AuthorityBoundCandidate SnapshotId)) :
    VotingWeight :=
  candidates.foldr
    (fun candidate aggregate =>
      safeMaximum candidate.declaredFaultBound aggregate)
    0

theorem candidate_bound_is_at_most_maximum
    {SnapshotId : Type}
    (candidate : AuthorityBoundCandidate SnapshotId)
    (candidates : List (AuthorityBoundCandidate SnapshotId))
    (present : candidate ∈ candidates) :
    candidate.declaredFaultBound ≤ maximumCandidateBound candidates := by
  induction candidates with
  | nil => cases present
  | cons head tail inductionHypothesis =>
      cases present with
      | head =>
          exact left_is_at_most_maximum
            candidate.declaredFaultBound
            (maximumCandidateBound tail)
      | tail _ presentInTail =>
          exact Nat.le_trans
            (inductionHypothesis presentInTail)
            (right_is_at_most_maximum
              head.declaredFaultBound
              (maximumCandidateBound tail))

theorem conservative_witness_makes_maximum_safe
    {SnapshotId : Type}
    (actualFaultWeight : VotingWeight)
    (witness : AuthorityBoundCandidate SnapshotId)
    (candidates : List (AuthorityBoundCandidate SnapshotId))
    (present : witness ∈ candidates)
    (conservative :
      actualFaultWeight ≤ witness.declaredFaultBound) :
    actualFaultWeight ≤ maximumCandidateBound candidates :=
  Nat.le_trans conservative
    (candidate_bound_is_at_most_maximum
      witness candidates present)

theorem adding_candidate_does_not_decrease_maximum
    {SnapshotId : Type}
    (candidate : AuthorityBoundCandidate SnapshotId)
    (candidates : List (AuthorityBoundCandidate SnapshotId)) :
    maximumCandidateBound candidates ≤
      maximumCandidateBound (candidate :: candidates) := by
  change
    maximumCandidateBound candidates ≤
      safeMaximum
        candidate.declaredFaultBound
        (maximumCandidateBound candidates)
  exact right_is_at_most_maximum
    candidate.declaredFaultBound
    (maximumCandidateBound candidates)

theorem minimum_aggregation_can_understate_actual_fault_weight :
    ∃ actualFaultWeight lowReport conservativeReport : VotingWeight,
      lowReport < actualFaultWeight ∧
      actualFaultWeight ≤ conservativeReport ∧
      Nat.min lowReport conservativeReport < actualFaultWeight := by
  exact ⟨
    1,
    0,
    1,
    Nat.zero_lt_succ 0,
    Nat.le_refl 1,
    Nat.zero_lt_succ 0
  ⟩

theorem partial_authority_search_can_pass_before_final_bound_fails :
    ∃ totalWeight quorumWeight partialBound expandedBound : VotingWeight,
      partialBound ≤ expandedBound ∧
      totalWeight + partialBound <
        quorumWeight + quorumWeight ∧
      ¬ totalWeight + expandedBound <
        quorumWeight + quorumWeight := by
  exact ⟨
    3,
    2,
    0,
    1,
    Nat.zero_le 1,
    Nat.lt_succ_self 3,
    Nat.lt_irrefl 4
  ⟩

theorem authentic_reports_without_conservative_witness_are_insufficient
    (actualFaultWeight aggregateBound : VotingWeight)
    (authenticReports : Prop)
    (authentic : authenticReports)
    (understated : aggregateBound < actualFaultWeight) :
    authenticReports ∧
    ¬ actualFaultWeight ≤ aggregateBound := by
  exact ⟨authentic, Nat.not_le_of_lt understated⟩

end ASPProof.SearchRouteAdmissionRetryCacheRejoinMultiAuthorityFaultAggregation
