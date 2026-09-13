-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteDAG

namespace SearchRouteEvidenceGraphRouter

open SearchRouteCost SearchRouteDAG

inductive CapabilityState where
  | ready
  | recoverable
  | unavailable
  deriving DecidableEq, Repr

structure EvidenceGraphSnapshot where
  rootDigest : Nat
  providerDigest : Nat
  queryDigest : Nat
  deriving DecidableEq, Repr

structure ExecutableGraphCandidate where
  graphCandidate : GraphCandidate
  evidenceRootDigest : Nat
  providerDigest : Nat
  queryDigest : Nat
  routeDigest : Nat
  capabilityState : CapabilityState
  selectorExecutable : Bool
  projectionCompatible : Bool
  deriving DecidableEq, Repr

def ReceiptBound
    (snapshot : EvidenceGraphSnapshot)
    (candidate : ExecutableGraphCandidate) : Prop :=
  candidate.evidenceRootDigest = snapshot.rootDigest ∧
  candidate.providerDigest = snapshot.providerDigest ∧
  candidate.queryDigest = snapshot.queryDigest

def RuntimeReady (candidate : ExecutableGraphCandidate) : Prop :=
  candidate.capabilityState = .ready ∧
  candidate.selectorExecutable = true ∧
  candidate.projectionCompatible = true

def CertifiedFeasible
    (requirement : RouteRequirement)
    (budget : GraphRouteBudget)
    (snapshot : EvidenceGraphSnapshot)
    (candidate : ExecutableGraphCandidate) : Prop :=
  graphFeasibleB requirement budget candidate.graphCandidate = true ∧
  ReceiptBound snapshot candidate ∧
  RuntimeReady candidate

structure CertifiedSelection
    (requirement : RouteRequirement)
    (budget : GraphRouteBudget)
    (snapshot : EvidenceGraphSnapshot)
    (catalog : List ExecutableGraphCandidate) where
  selected : ExecutableGraphCandidate
  selectedMem : selected ∈ catalog
  selectedFeasible : CertifiedFeasible requirement budget snapshot selected
  noWorse :
    ∀ candidate,
      candidate ∈ catalog →
      CertifiedFeasible requirement budget snapshot candidate →
      graphLexNoWorse selected.graphCandidate candidate.graphCandidate

def CertifiedCatalogComplete
    (requirement : RouteRequirement)
    (budget : GraphRouteBudget)
    (snapshot : EvidenceGraphSnapshot)
    (candidateUniverse catalog : List ExecutableGraphCandidate) : Prop :=
  ∀ candidate,
    candidate ∈ candidateUniverse →
    CertifiedFeasible requirement budget snapshot candidate →
    candidate ∈ catalog

theorem certified_feasible_is_runtime_ready
    {requirement : RouteRequirement}
    {budget : GraphRouteBudget}
    {snapshot : EvidenceGraphSnapshot}
    {candidate : ExecutableGraphCandidate}
    (certified : CertifiedFeasible requirement budget snapshot candidate) :
    RuntimeReady candidate :=
  certified.2.2

theorem root_mismatch_rejects_candidate
    {requirement : RouteRequirement}
    {budget : GraphRouteBudget}
    {snapshot : EvidenceGraphSnapshot}
    {candidate : ExecutableGraphCandidate}
    (mismatch : candidate.evidenceRootDigest ≠ snapshot.rootDigest) :
    ¬ CertifiedFeasible requirement budget snapshot candidate := by
  intro certified
  exact mismatch certified.2.1.1

theorem recoverable_candidate_is_not_runtime_ready
    {candidate : ExecutableGraphCandidate}
    (recoverable : candidate.capabilityState = .recoverable) :
    ¬ RuntimeReady candidate := by
  intro ready
  simp [RuntimeReady, recoverable] at ready

theorem incompatible_projection_rejects_candidate
    {requirement : RouteRequirement}
    {budget : GraphRouteBudget}
    {snapshot : EvidenceGraphSnapshot}
    {candidate : ExecutableGraphCandidate}
    (incompatible : candidate.projectionCompatible = false) :
    ¬ CertifiedFeasible requirement budget snapshot candidate := by
  intro certified
  have compatible := certified.2.2.2.2
  simp [incompatible] at compatible

theorem graph_feasibility_does_not_imply_runtime_readiness
    (requirement : RouteRequirement)
    (budget : GraphRouteBudget)
    (candidate : GraphCandidate)
    (graphFeasible : graphFeasibleB requirement budget candidate = true) :
    ∃ (snapshot : EvidenceGraphSnapshot)
      (wrapped : ExecutableGraphCandidate),
      wrapped.graphCandidate = candidate ∧
      ReceiptBound snapshot wrapped ∧
      graphFeasibleB requirement budget wrapped.graphCandidate = true ∧
      ¬ RuntimeReady wrapped := by
  let snapshot : EvidenceGraphSnapshot := {
    rootDigest := 0
    providerDigest := 0
    queryDigest := 0
  }
  let wrapped : ExecutableGraphCandidate := {
    graphCandidate := candidate
    evidenceRootDigest := snapshot.rootDigest
    providerDigest := snapshot.providerDigest
    queryDigest := snapshot.queryDigest
    routeDigest := 0
    capabilityState := .recoverable
    selectorExecutable := false
    projectionCompatible := false
  }
  refine ⟨snapshot, wrapped, rfl, ?_, ?_, ?_⟩
  · simp [ReceiptBound, wrapped]
  · exact graphFeasible
  · simp [RuntimeReady, wrapped]

theorem certified_selection_is_catalog_optimal
    {requirement : RouteRequirement}
    {budget : GraphRouteBudget}
    {snapshot : EvidenceGraphSnapshot}
    {catalog : List ExecutableGraphCandidate}
    (selection : CertifiedSelection requirement budget snapshot catalog) :
    ∀ candidate,
      candidate ∈ catalog →
      CertifiedFeasible requirement budget snapshot candidate →
      graphLexNoWorse
        selection.selected.graphCandidate
        candidate.graphCandidate :=
  selection.noWorse

theorem complete_certified_selection_is_universe_optimal
    {requirement : RouteRequirement}
    {budget : GraphRouteBudget}
    {snapshot : EvidenceGraphSnapshot}
    {candidateUniverse catalog : List ExecutableGraphCandidate}
    (selection : CertifiedSelection requirement budget snapshot catalog)
    (complete :
      CertifiedCatalogComplete
        requirement budget snapshot candidateUniverse catalog) :
    ∀ candidate,
      candidate ∈ candidateUniverse →
      CertifiedFeasible requirement budget snapshot candidate →
      graphLexNoWorse
        selection.selected.graphCandidate
        candidate.graphCandidate := by
  intro candidate inUniverse feasible
  exact selection.noWorse candidate (complete candidate inUniverse feasible) feasible

end SearchRouteEvidenceGraphRouter
