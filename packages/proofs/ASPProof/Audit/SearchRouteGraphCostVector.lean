import ASPProof.Audit.Core
import ASPProof.SearchRouteGraphCostVector

namespace ASPProof.Audit.SearchRouteGraphCostVector

open Lean
open ASPProof.Audit.Core

def targets : List Target :=
  [
    { name := ``_root_.SearchRouteGraphCostVector.round_transition_refl
      theoremFamily := "round-transition-reflexivity"
      rfcClauseIds := ["ASP-RFC-10.05-GCV-ORDER"] },
    { name := ``_root_.SearchRouteGraphCostVector.round_transition_trans
      theoremFamily := "round-transition-transitivity"
      rfcClauseIds := ["ASP-RFC-10.05-GCV-ORDER"] },
    { name := ``_root_.SearchRouteGraphCostVector.token_tail_refl
      theoremFamily := "token-tail-reflexivity"
      rfcClauseIds := ["ASP-RFC-10.05-GCV-ORDER"] },
    { name := ``_root_.SearchRouteGraphCostVector.token_tail_trans
      theoremFamily := "token-tail-transitivity"
      rfcClauseIds := ["ASP-RFC-10.05-GCV-ORDER"] },
    { name := ``_root_.SearchRouteGraphCostVector.cost_vector_refl
      theoremFamily := "cost-vector-reflexivity"
      rfcClauseIds := ["ASP-RFC-10.05-GCV-ORDER"] },
    { name := ``_root_.SearchRouteGraphCostVector.cost_vector_trans
      theoremFamily := "cost-vector-transitivity"
      rfcClauseIds := ["ASP-RFC-10.05-GCV-ORDER"] },
    { name := ``_root_.SearchRouteGraphCostVector.candidate_order_equivalent
      theoremFamily := "candidate-order-equivalence"
      rfcClauseIds := [
        "ASP-RFC-10.05-GCV-EQUIVALENCE",
        "ASP-RFC-10.05-GCV-PROJECTION"
      ] },
    { name := ``_root_.SearchRouteGraphCostVector.certified_cost_frontier_selection_is_global
      theoremFamily := "vector-frontier-global-optimality"
      rfcClauseIds := [
        "ASP-RFC-10.05-GCV-FRONTIER",
        "ASP-RFC-10.05-GCV-GLOBAL"
      ] },
    { name := ``_root_.SearchRouteGraphCostVector.vector_global_optimality_implies_graph_global_optimality
      theoremFamily := "graph-order-transfer"
      rfcClauseIds := [
        "ASP-RFC-10.05-GCV-EQUIVALENCE",
        "ASP-RFC-10.05-GCV-GLOBAL"
      ] },
    { name := ``_root_.SearchRouteGraphCostVector.hop_bound_alone_does_not_close_frontier
      theoremFamily := "hop-only-counterexample"
      rfcClauseIds := ["ASP-RFC-10.05-GCV-HOP-ONLY"] }
  ]

def auditJson : Elab.Term.TermElabM Json :=
  proofAuditJson
    "ASPProof.SearchRouteGraphCostVector"
    "packages/proofs/ASPProof/SearchRouteGraphCostVector.lean"
    targets

end ASPProof.Audit.SearchRouteGraphCostVector
