-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteInspectTraceCatalog
import ASPProof.SearchRouteInspectScheduler
import ASPProof.SearchRouteDAGEnumeration

namespace SearchRouteInspectCertifiedLedger

open SearchRouteCost
open SearchRouteDAG
open SearchRouteBranchBound
open SearchRouteInspectLoop
open SearchRouteInspectScheduler
open SearchRouteInspectTraceCatalog

structure RankedDAGSnapshot where
  digest : Nat
  graph : RankedDAG

structure CertifiedTraceLedger
    (initial : InspectLoopState)
    (snapshot : RankedDAGSnapshot)
    (source target : Fin snapshot.graph.nodeCount)
    (requirement : RouteRequirement)
    (budget : GraphRouteBudget)
    (selected : GraphCandidate) where
  candidateUniverse : CandidateUniverse
  universeSnapshotBound :
    candidateUniverse.snapshotDigest = snapshot.digest
  visibleCatalog :
    List
      (RealizedTraceCatalogEntry
        initial snapshot.graph source target)
  visibleFeasible :
    ∀ candidate ∈ projectTraceCatalog visibleCatalog,
      graphFeasibleB requirement budget candidate = true
  frontiers : List CertifiedPartialFrontier
  coverage :
    CoverageReceipt
      candidateUniverse
      (projectTraceCatalog visibleCatalog)
  coverageFrontiers :
    coverage.deferred =
      frontiers.map CertifiedPartialFrontier.toCertifiedFrontier
  universeComplete :
    CandidateComplete
      snapshot.graph
      source
      target
      requirement
      budget
      candidateUniverse.candidates
  selectedByRouter :
    chooseBestGraphCandidate
        requirement
        budget
        (projectTraceCatalog visibleCatalog) =
      some selected
  scheduleComplete : ScheduleComplete selected frontiers

theorem certified_trace_ledger_snapshot_is_bound
    {initial : InspectLoopState}
    {snapshot : RankedDAGSnapshot}
    {source target : Fin snapshot.graph.nodeCount}
    {requirement : RouteRequirement}
    {budget : GraphRouteBudget}
    {selected : GraphCandidate}
    (ledger :
      CertifiedTraceLedger
        initial snapshot source target requirement budget selected) :
    ledger.coverage.claimedSnapshotDigest = snapshot.digest := by
  exact ledger.coverage.snapshotBound.trans ledger.universeSnapshotBound

theorem certified_trace_ledger_visible_is_optimal
    {initial : InspectLoopState}
    {snapshot : RankedDAGSnapshot}
    {source target : Fin snapshot.graph.nodeCount}
    {requirement : RouteRequirement}
    {budget : GraphRouteBudget}
    {selected : GraphCandidate}
    (ledger :
      CertifiedTraceLedger
        initial snapshot source target requirement budget selected) :
    VisibleOptimal
      selected
      (projectTraceCatalog ledger.visibleCatalog) := by
  have selectedReceipt :=
    chooseBestGraphCandidate_mem_and_feasible
      requirement
      budget
      (projectTraceCatalog ledger.visibleCatalog)
      selected
      ledger.selectedByRouter
  refine ⟨selectedReceipt.1, ?_⟩
  intro candidate candidateVisible
  exact
    chooseBestGraphCandidate_is_lex_optimal
      requirement
      budget
      (projectTraceCatalog ledger.visibleCatalog)
      selected
      ledger.selectedByRouter
      candidate
      candidateVisible
      (ledger.visibleFeasible candidate candidateVisible)

theorem certified_trace_ledger_selected_has_provenance
    {initial : InspectLoopState}
    {snapshot : RankedDAGSnapshot}
    {source target : Fin snapshot.graph.nodeCount}
    {requirement : RouteRequirement}
    {budget : GraphRouteBudget}
    {selected : GraphCandidate}
    (ledger :
      CertifiedTraceLedger
        initial snapshot source target requirement budget selected) :
    ∃ entry ∈ ledger.visibleCatalog, entry.candidate = selected :=
  selected_candidate_has_trace_provenance ledger.selectedByRouter

theorem certified_trace_ledger_selected_realizes
    {initial : InspectLoopState}
    {snapshot : RankedDAGSnapshot}
    {source target : Fin snapshot.graph.nodeCount}
    {requirement : RouteRequirement}
    {budget : GraphRouteBudget}
    {selected : GraphCandidate}
    (ledger :
      CertifiedTraceLedger
        initial snapshot source target requirement budget selected) :
    Realizes snapshot.graph source target selected := by
  rcases certified_trace_ledger_selected_has_provenance ledger with
    ⟨entry, _, entryProjectsToSelected⟩
  rw [← entryProjectsToSelected]
  exact entry.realization

theorem certified_trace_ledger_selects_global_driver
    {initial : InspectLoopState}
    {snapshot : RankedDAGSnapshot}
    {source target : Fin snapshot.graph.nodeCount}
    {requirement : RouteRequirement}
    {budget : GraphRouteBudget}
    {selected : GraphCandidate}
    (ledger :
      CertifiedTraceLedger
        initial snapshot source target requirement budget selected) :
    (∃ entry ∈ ledger.visibleCatalog, entry.candidate = selected) ∧
      Realizes snapshot.graph source target selected ∧
      ∀ candidate,
        Realizes snapshot.graph source target candidate →
          graphFeasibleB requirement budget candidate = true →
            graphLexNoWorse selected candidate := by
  refine ⟨
    certified_trace_ledger_selected_has_provenance ledger,
    certified_trace_ledger_selected_realizes ledger,
    ?_
  ⟩
  intro candidate candidateRealizes candidateFeasible
  have candidateInUniverse :
      candidate ∈ ledger.candidateUniverse.candidates :=
    ledger.universeComplete
      candidate
      candidateRealizes
      candidateFeasible
  exact
    completed_inspect_schedule_is_globally_optimal
      (certified_trace_ledger_visible_is_optimal ledger)
      ledger.coverage
      ledger.coverageFrontiers
      ledger.scheduleComplete
      candidate
      candidateInUniverse

theorem empty_schedule_is_complete (selected : GraphCandidate) :
    ScheduleComplete selected [] := by
  rfl

theorem schedule_closure_does_not_imply_candidate_completeness :
    ScheduleComplete
        SearchRouteDAGEnumeration.generatedGraphOnlyCandidate
        [] ∧
      ¬CandidateComplete
        SearchRouteDAGEnumeration.oneNodeGraph
        SearchRouteDAGEnumeration.onlyNode
        SearchRouteDAGEnumeration.onlyNode
        nonGraphRequirement
        generousGraphBudget
        SearchRouteDAGEnumeration.graphOnlyCandidates := by
  exact ⟨
    empty_schedule_is_complete
      SearchRouteDAGEnumeration.generatedGraphOnlyCandidate,
    SearchRouteDAGEnumeration.graph_only_candidates_are_not_candidate_complete
  ⟩

end SearchRouteInspectCertifiedLedger
