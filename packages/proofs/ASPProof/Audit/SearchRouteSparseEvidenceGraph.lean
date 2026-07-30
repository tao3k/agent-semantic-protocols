import ASPProof.Audit.Core
import ASPProof.SearchRouteSparseEvidenceGraph

namespace ASPProof.Audit.SearchRouteSparseEvidenceGraph

open Lean
open ASPProof.Audit.Core

def targets : List Target :=
  [
    { name := ``_root_.SearchRouteSparseEvidenceGraph.sparse_walk_lifts_to_full_graph
      theoremFamily := "walk-soundness"
      rfcClauseIds := ["ASP-RFC-10.05-SEG-EDGE-SOUND"] },
    { name := ``_root_.SearchRouteSparseEvidenceGraph.certified_sparse_route_is_globally_optimal
      theoremFamily := "global-sparse-route-optimality"
      rfcClauseIds := [
        "ASP-RFC-10.05-SEG-DISTANCE-COMPLETE",
        "ASP-RFC-10.05-SEG-GLOBAL"
      ] },
    { name := ``_root_.SearchRouteSparseEvidenceGraph.counterexample_two_hop_walk
      theoremFamily := "counterexample-sparse-walk"
      rfcClauseIds := ["ASP-RFC-10.05-SEG-COUNTEREXAMPLE"] },
    { name := ``_root_.SearchRouteSparseEvidenceGraph.counterexample_direct_walk
      theoremFamily := "counterexample-direct-walk"
      rfcClauseIds := ["ASP-RFC-10.05-SEG-COUNTEREXAMPLE"] },
    { name := ``_root_.SearchRouteSparseEvidenceGraph.counterexample_projection_is_edge_sound
      theoremFamily := "counterexample-edge-soundness"
      rfcClauseIds := [
        "ASP-RFC-10.05-SEG-COUNTEREXAMPLE",
        "ASP-RFC-10.05-SEG-EDGE-SOUND"
      ] },
    { name := ``_root_.SearchRouteSparseEvidenceGraph.edge_sound_sparse_graph_can_hide_shorter_path
      theoremFamily := "edge-soundness-insufficiency"
      rfcClauseIds := [
        "ASP-RFC-10.05-SEG-COUNTEREXAMPLE",
        "ASP-RFC-10.05-SEG-EDGE-SOUND"
      ] }
  ]

def auditJson : Elab.Term.TermElabM Json :=
  proofAuditJson
    "ASPProof.SearchRouteSparseEvidenceGraph"
    "packages/proofs/ASPProof/SearchRouteSparseEvidenceGraph.lean"
    targets

end ASPProof.Audit.SearchRouteSparseEvidenceGraph
