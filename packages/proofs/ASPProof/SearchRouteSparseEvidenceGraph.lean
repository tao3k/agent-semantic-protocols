-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace SearchRouteSparseEvidenceGraph

structure EvidenceEdge where
  source : Nat
  target : Nat
  cost : Nat
  evidenceDigest : Nat
  deriving DecidableEq, Repr

structure EvidenceGraph where
  edges : List EvidenceEdge
  deriving DecidableEq, Repr

inductive Walk
    (graph : EvidenceGraph) :
    Nat → Nat → List EvidenceEdge → Prop where
  | nil (node : Nat) :
      Walk graph node node []
  | cons
      (edge : EvidenceEdge)
      {target : Nat}
      {tail : List EvidenceEdge}
      (edgeMem : edge ∈ graph.edges)
      (rest : Walk graph edge.target target tail) :
      Walk graph edge.source target (edge :: tail)

def PathCost (path : List EvidenceEdge) : Nat :=
  path.foldl (fun total edge => total + edge.cost) 0

def EdgeSound
    (full sparse : EvidenceGraph) : Prop :=
  ∀ edge, edge ∈ sparse.edges → edge ∈ full.edges

def SparseOptimal
    (sparse : EvidenceGraph)
    (source target : Nat)
    (selected : List EvidenceEdge) : Prop :=
  Walk sparse source target selected ∧
  ∀ candidate,
    Walk sparse source target candidate →
    PathCost selected ≤ PathCost candidate

def DistanceComplete
    (full sparse : EvidenceGraph)
    (source target : Nat) : Prop :=
  ∀ fullPath,
    Walk full source target fullPath →
    ∃ sparsePath,
      Walk sparse source target sparsePath ∧
      PathCost sparsePath ≤ PathCost fullPath

structure CertifiedSparseRoute
    (activeGeneration : Nat)
    (full sparse : EvidenceGraph)
    (source target : Nat) where
  claimedGeneration : Nat
  generationBound : claimedGeneration = activeGeneration
  fullRootDigest : Nat
  sparseRootDigest : Nat
  selected : List EvidenceEdge
  edgeSound : EdgeSound full sparse
  sparseOptimal : SparseOptimal sparse source target selected
  distanceComplete : DistanceComplete full sparse source target

theorem sparse_walk_lifts_to_full_graph
    {full sparse : EvidenceGraph}
    (sound : EdgeSound full sparse)
    {source target : Nat}
    {path : List EvidenceEdge}
    (walk : Walk sparse source target path) :
    Walk full source target path := by
  induction walk with
  | nil node =>
      exact Walk.nil (graph := full) node
  | cons edge edgeMem rest inductionHypothesis =>
      exact Walk.cons
        (graph := full)
        edge
        (sound edge edgeMem)
        inductionHypothesis

theorem certified_sparse_route_is_globally_optimal
    {activeGeneration : Nat}
    {full sparse : EvidenceGraph}
    {source target : Nat}
    (certificate :
      CertifiedSparseRoute
        activeGeneration full sparse source target) :
    ∀ fullPath,
      Walk full source target fullPath →
      PathCost certificate.selected ≤ PathCost fullPath := by
  intro fullPath fullWalk
  obtain ⟨sparsePath, sparseWalk, projectedNoWorse⟩ :=
    certificate.distanceComplete fullPath fullWalk
  exact Nat.le_trans
    (certificate.sparseOptimal.2 sparsePath sparseWalk)
    projectedNoWorse

def edge01 : EvidenceEdge := {
  source := 0
  target := 1
  cost := 1
  evidenceDigest := 101
}

def edge12 : EvidenceEdge := {
  source := 1
  target := 2
  cost := 1
  evidenceDigest := 112
}

def edge02 : EvidenceEdge := {
  source := 0
  target := 2
  cost := 1
  evidenceDigest := 102
}

def fullCounterexampleGraph : EvidenceGraph := {
  edges := [edge01, edge12, edge02]
}

def sparseCounterexampleGraph : EvidenceGraph := {
  edges := [edge01, edge12]
}

theorem counterexample_two_hop_walk :
    Walk sparseCounterexampleGraph 0 2 [edge01, edge12] := by
  apply Walk.cons (graph := sparseCounterexampleGraph) edge01
  · simp [sparseCounterexampleGraph]
  · apply Walk.cons (graph := sparseCounterexampleGraph) edge12
    · simp [sparseCounterexampleGraph]
    · exact Walk.nil (graph := sparseCounterexampleGraph) 2

theorem counterexample_direct_walk :
    Walk fullCounterexampleGraph 0 2 [edge02] := by
  apply Walk.cons (graph := fullCounterexampleGraph) edge02
  · simp [fullCounterexampleGraph]
  · exact Walk.nil (graph := fullCounterexampleGraph) 2

theorem counterexample_projection_is_edge_sound :
    EdgeSound fullCounterexampleGraph sparseCounterexampleGraph := by
  intro edge member
  simp [sparseCounterexampleGraph] at member
  rcases member with rfl | rfl
  · simp [fullCounterexampleGraph]
  · simp [fullCounterexampleGraph]

theorem edge_sound_sparse_graph_can_hide_shorter_path :
    EdgeSound fullCounterexampleGraph sparseCounterexampleGraph ∧
    Walk sparseCounterexampleGraph 0 2 [edge01, edge12] ∧
    Walk fullCounterexampleGraph 0 2 [edge02] ∧
    PathCost [edge02] < PathCost [edge01, edge12] := by
  refine ⟨
    counterexample_projection_is_edge_sound,
    counterexample_two_hop_walk,
    counterexample_direct_walk,
    ?_
  ⟩
  decide

end SearchRouteSparseEvidenceGraph
