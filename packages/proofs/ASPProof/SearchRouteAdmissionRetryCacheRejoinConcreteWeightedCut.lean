-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteAdmissionRetryCacheRejoinWeightedAuthorityQuantile

namespace ASPProof.SearchRouteAdmissionRetryCacheRejoinConcreteWeightedCut

open ASPProof.SearchRouteAdmissionRetryCacheRejoinCanonicalMembership

structure WeightedAuthorityReport (AuthorityId : Type) where
  authorityId : AuthorityId
  boundValue : VotingWeight
  votingWeight : VotingWeight

def totalVotingWeight
    {AuthorityId : Type} :
    List (WeightedAuthorityReport AuthorityId) → VotingWeight
  | [] => 0
  | report :: rest =>
      report.votingWeight + totalVotingWeight rest

def DistinctAuthorities
    {AuthorityId : Type}
    (reports : List (WeightedAuthorityReport AuthorityId)) : Prop :=
  reports.Pairwise
    (fun left right =>
      left.authorityId ≠ right.authorityId)

def ValuesStrictlyBelow
    {AuthorityId : Type}
    (threshold : VotingWeight) :
    List (WeightedAuthorityReport AuthorityId) → Prop
  | [] => True
  | report :: rest =>
      report.boundValue < threshold ∧
        ValuesStrictlyBelow threshold rest

def ValuesStrictlyAbove
    {AuthorityId : Type}
    (threshold : VotingWeight) :
    List (WeightedAuthorityReport AuthorityId) → Prop
  | [] => True
  | report :: rest =>
      threshold < report.boundValue ∧
        ValuesStrictlyAbove threshold rest

def ValuesAtMost
    {AuthorityId : Type}
    (upperBound : VotingWeight) :
    List (WeightedAuthorityReport AuthorityId) → Prop
  | [] => True
  | report :: rest =>
      report.boundValue ≤ upperBound ∧
        ValuesAtMost upperBound rest

def ValuesAtLeast
    {AuthorityId : Type}
    (lowerBound : VotingWeight) :
    List (WeightedAuthorityReport AuthorityId) → Prop
  | [] => True
  | report :: rest =>
      lowerBound ≤ report.boundValue ∧
        ValuesAtLeast lowerBound rest

structure ConcreteWeightedCut
    {AuthorityId : Type}
    (reports : List (WeightedAuthorityReport AuthorityId))
    (cutWeight : VotingWeight) where
  lowerReports : List (WeightedAuthorityReport AuthorityId)
  selectedReport : WeightedAuthorityReport AuthorityId
  upperReports : List (WeightedAuthorityReport AuthorityId)
  decomposition :
    reports =
      lowerReports ++ selectedReport :: upperReports
  lowerWeightLtCut :
    totalVotingWeight lowerReports < cutWeight
  cutLeLowerWithSelected :
    cutWeight ≤
      totalVotingWeight lowerReports +
        selectedReport.votingWeight
  lowerValuesLeSelected :
    ValuesAtMost
      selectedReport.boundValue lowerReports
  selectedLeUpperValues :
    ValuesAtLeast
      selectedReport.boundValue upperReports

theorem totalVotingWeight_append
    {AuthorityId : Type}
    (left right : List (WeightedAuthorityReport AuthorityId)) :
    totalVotingWeight (left ++ right) =
      totalVotingWeight left + totalVotingWeight right := by
  induction left with
  | nil =>
      change
        totalVotingWeight right =
          0 + totalVotingWeight right
      exact (Nat.zero_add _).symm
  | cons report rest inductionHypothesis =>
      change
        report.votingWeight +
            totalVotingWeight (rest ++ right) =
          report.votingWeight +
            totalVotingWeight rest +
            totalVotingWeight right
      rw [inductionHypothesis, Nat.add_assoc]

theorem concrete_cut_conserves_total_weight
    {AuthorityId : Type}
    {reports : List (WeightedAuthorityReport AuthorityId)}
    {cutWeight : VotingWeight}
    (certificate : ConcreteWeightedCut reports cutWeight) :
    totalVotingWeight reports =
      totalVotingWeight certificate.lowerReports +
        certificate.selectedReport.votingWeight +
        totalVotingWeight certificate.upperReports := by
  calc
    totalVotingWeight reports =
        totalVotingWeight
          (certificate.lowerReports ++
            certificate.selectedReport ::
              certificate.upperReports) := by
              exact
                congrArg totalVotingWeight
                  certificate.decomposition
    _ =
        totalVotingWeight certificate.lowerReports +
          totalVotingWeight
            (certificate.selectedReport ::
              certificate.upperReports) :=
      totalVotingWeight_append
        certificate.lowerReports
        (certificate.selectedReport ::
          certificate.upperReports)
    _ =
        totalVotingWeight certificate.lowerReports +
          (certificate.selectedReport.votingWeight +
            totalVotingWeight certificate.upperReports) := rfl
    _ =
        totalVotingWeight certificate.lowerReports +
          certificate.selectedReport.votingWeight +
          totalVotingWeight certificate.upperReports := by
      rw [Nat.add_assoc]

theorem concrete_cut_arithmetic_witnesses
    {AuthorityId : Type}
    {reports : List (WeightedAuthorityReport AuthorityId)}
    {cutWeight : VotingWeight}
    (certificate : ConcreteWeightedCut reports cutWeight) :
    cutWeight ≤
        totalVotingWeight certificate.lowerReports +
          certificate.selectedReport.votingWeight ∧
      totalVotingWeight reports <
        cutWeight +
          (certificate.selectedReport.votingWeight +
            totalVotingWeight certificate.upperReports) := by
  constructor
  · exact certificate.cutLeLowerWithSelected
  · rw [concrete_cut_conserves_total_weight certificate]
    rw [Nat.add_assoc]
    exact
      Nat.add_lt_add_right
        certificate.lowerWeightLtCut
        (certificate.selectedReport.votingWeight +
          totalVotingWeight certificate.upperReports)

theorem valuesAtMost_then_strictlyBelow
    {AuthorityId : Type}
    (upperBound threshold : VotingWeight)
    (reports : List (WeightedAuthorityReport AuthorityId))
    (ordered : ValuesAtMost upperBound reports)
    (upperBelow : upperBound < threshold) :
    ValuesStrictlyBelow threshold reports := by
  induction reports with
  | nil =>
      exact True.intro
  | cons report rest inductionHypothesis =>
      change
        report.boundValue ≤ upperBound ∧
          ValuesAtMost upperBound rest at ordered
      change
        report.boundValue < threshold ∧
          ValuesStrictlyBelow threshold rest
      exact ⟨
        Nat.lt_of_le_of_lt ordered.1 upperBelow,
        inductionHypothesis ordered.2
      ⟩

theorem valuesAtLeast_then_strictlyAbove
    {AuthorityId : Type}
    (lowerBound threshold : VotingWeight)
    (reports : List (WeightedAuthorityReport AuthorityId))
    (ordered : ValuesAtLeast lowerBound reports)
    (thresholdBelow : threshold < lowerBound) :
    ValuesStrictlyAbove threshold reports := by
  induction reports with
  | nil =>
      exact True.intro
  | cons report rest inductionHypothesis =>
      change
        lowerBound ≤ report.boundValue ∧
          ValuesAtLeast lowerBound rest at ordered
      change
        threshold < report.boundValue ∧
          ValuesStrictlyAbove threshold rest
      exact ⟨
        Nat.lt_of_lt_of_le thresholdBelow ordered.1,
        inductionHypothesis ordered.2
      ⟩

theorem selected_below_threshold_covers_lower_partition
    {AuthorityId : Type}
    {reports : List (WeightedAuthorityReport AuthorityId)}
    {cutWeight threshold : VotingWeight}
    (certificate : ConcreteWeightedCut reports cutWeight)
    (selectedBelow :
      certificate.selectedReport.boundValue < threshold) :
    ValuesStrictlyBelow
        threshold certificate.lowerReports ∧
      certificate.selectedReport.boundValue < threshold := by
  constructor
  · exact
      valuesAtMost_then_strictlyBelow
        certificate.selectedReport.boundValue
        threshold
        certificate.lowerReports
        certificate.lowerValuesLeSelected
        selectedBelow
  · exact selectedBelow

theorem selected_above_threshold_covers_upper_partition
    {AuthorityId : Type}
    {reports : List (WeightedAuthorityReport AuthorityId)}
    {cutWeight threshold : VotingWeight}
    (certificate : ConcreteWeightedCut reports cutWeight)
    (selectedAbove :
      threshold < certificate.selectedReport.boundValue) :
    threshold < certificate.selectedReport.boundValue ∧
      ValuesStrictlyAbove
        threshold certificate.upperReports := by
  constructor
  · exact selectedAbove
  · exact
      valuesAtLeast_then_strictlyAbove
        certificate.selectedReport.boundValue
        threshold
        certificate.upperReports
        certificate.selectedLeUpperValues
        selectedAbove

theorem duplicate_authority_cannot_form_distinct_admission
    {AuthorityId : Type}
    (report : WeightedAuthorityReport AuthorityId) :
    ¬ DistinctAuthorities [report, report] := by
  intro distinct
  cases distinct with
  | cons headDistinct tailDistinct =>
      exact
        (headDistinct
          report
          (List.mem_cons_self))
          rfl

theorem duplicate_report_can_inflate_total_weight :
    ∃ report : WeightedAuthorityReport Nat,
      totalVotingWeight [report, report] >
        totalVotingWeight [report] := by
  let report : WeightedAuthorityReport Nat := {
    authorityId := 7
    boundValue := 3
    votingWeight := 2
  }
  exact ⟨report, by decide⟩

theorem cumulative_cut_without_value_order_is_insufficient :
    ∃ lowerReports : List (WeightedAuthorityReport Nat),
      ∃ selectedReport upperReport : WeightedAuthorityReport Nat,
        ∃ cutWeight : VotingWeight,
          totalVotingWeight lowerReports < cutWeight ∧
          cutWeight ≤
            totalVotingWeight lowerReports +
              selectedReport.votingWeight ∧
          selectedReport.boundValue >
            upperReport.boundValue := by
  let high : WeightedAuthorityReport Nat := {
    authorityId := 1
    boundValue := 10
    votingWeight := 4
  }
  let low : WeightedAuthorityReport Nat := {
    authorityId := 2
    boundValue := 0
    votingWeight := 6
  }
  exact ⟨[], high, low, 4,
    by decide, by decide, by decide⟩

end ASPProof.SearchRouteAdmissionRetryCacheRejoinConcreteWeightedCut
