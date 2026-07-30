import ASPProof.SearchRouteInspectTraceCatalog

namespace SearchRouteSparseFrontierCertificate

open SearchRouteDAG

structure CertifiedOmittedFrontier where
  candidates : List GraphCandidate
  lowerBound : GraphCandidate
  lowerBoundValid :
    ∀ candidate,
      candidate ∈ candidates →
      graphLexNoWorse lowerBound candidate

def SparseCoverage
    (fullCatalog visible : List GraphCandidate)
    (frontiers : List CertifiedOmittedFrontier) : Prop :=
  ∀ candidate,
    candidate ∈ fullCatalog →
    candidate ∈ visible ∨
      ∃ frontier,
        frontier ∈ frontiers ∧
        candidate ∈ frontier.candidates

def VisibleOptimal
    (selected : GraphCandidate)
    (visible : List GraphCandidate) : Prop :=
  ∀ candidate,
    candidate ∈ visible →
    graphLexNoWorse selected candidate

def FrontiersClosed
    (selected : GraphCandidate)
    (frontiers : List CertifiedOmittedFrontier) : Prop :=
  ∀ frontier,
    frontier ∈ frontiers →
    graphLexNoWorse selected frontier.lowerBound

def GlobalOptimal
    (selected : GraphCandidate)
    (fullCatalog : List GraphCandidate) : Prop :=
  ∀ candidate,
    candidate ∈ fullCatalog →
    graphLexNoWorse selected candidate

def CatalogDistanceComplete
    (fullCatalog sparseCatalog : List GraphCandidate) : Prop :=
  ∀ fullCandidate,
    fullCandidate ∈ fullCatalog →
    ∃ sparseCandidate,
      sparseCandidate ∈ sparseCatalog ∧
      graphLexNoWorse sparseCandidate fullCandidate

theorem certified_frontier_selection_is_global
    {selected : GraphCandidate}
    {fullCatalog visible : List GraphCandidate}
    {frontiers : List CertifiedOmittedFrontier}
    (visibleOptimal : VisibleOptimal selected visible)
    (coverage : SparseCoverage fullCatalog visible frontiers)
    (closed : FrontiersClosed selected frontiers) :
    GlobalOptimal selected fullCatalog := by
  intro candidate inFull
  rcases coverage candidate inFull with inVisible | ⟨frontier, frontierMem, candidateMem⟩
  · exact visibleOptimal candidate inVisible
  · exact SearchRouteInspectTraceCatalog.graph_candidate_lex_trans
      (closed frontier frontierMem)
      (frontier.lowerBoundValid candidate candidateMem)

theorem certified_frontiers_imply_catalog_distance_completeness
    {selected : GraphCandidate}
    {fullCatalog visible : List GraphCandidate}
    {frontiers : List CertifiedOmittedFrontier}
    (selectedMem : selected ∈ visible)
    (visibleOptimal : VisibleOptimal selected visible)
    (coverage : SparseCoverage fullCatalog visible frontiers)
    (closed : FrontiersClosed selected frontiers) :
    CatalogDistanceComplete fullCatalog visible := by
  have global :=
    certified_frontier_selection_is_global
      visibleOptimal coverage closed
  intro candidate inFull
  exact ⟨selected, selectedMem, global candidate inFull⟩

theorem distance_complete_sparse_selection_is_global
    {selected : GraphCandidate}
    {fullCatalog sparseCatalog : List GraphCandidate}
    (sparseOptimal : VisibleOptimal selected sparseCatalog)
    (distanceComplete :
      CatalogDistanceComplete fullCatalog sparseCatalog) :
    GlobalOptimal selected fullCatalog := by
  intro fullCandidate inFull
  obtain ⟨sparseCandidate, inSparse, sparseNoWorse⟩ :=
    distanceComplete fullCandidate inFull
  exact SearchRouteInspectTraceCatalog.graph_candidate_lex_trans
    (sparseOptimal sparseCandidate inSparse)
    sparseNoWorse

theorem uncovered_hidden_candidate_invalidates_global_claim
    (selected hidden : GraphCandidate)
    (hiddenIsBetter : ¬ graphLexNoWorse selected hidden) :
    ¬ GlobalOptimal selected [selected, hidden] := by
  intro global
  exact hiddenIsBetter (global hidden (by simp))

end SearchRouteSparseFrontierCertificate
