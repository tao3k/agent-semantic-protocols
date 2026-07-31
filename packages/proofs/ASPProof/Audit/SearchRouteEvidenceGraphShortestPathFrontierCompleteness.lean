import ASPProof.Audit.Core
import ASPProof.SearchRouteEvidenceGraphShortestPathFrontierCompleteness

namespace ASPProof.Audit

def writeReceipt
    (path : System.FilePath)
    (auditJson : Lean.Elab.TermElabM Lean.Json) :
    Lean.Elab.Command.CommandElabM Unit := do
  let audit ← Lean.Elab.Command.liftTermElabM auditJson
  IO.FS.writeFile path (audit.pretty ++ "\n")
  Lean.logInfo m!"wrote {path}"

end ASPProof.Audit

namespace ASPProof.Audit.SearchRouteEvidenceGraphShortestPathFrontierCompleteness

open ASPProof.Audit.Core
open ASPProof.SearchRouteEvidenceGraphShortestPathFrontierCompleteness

def targets : List Target := [
  Target.mk ``stay_path_has_zero_hops
    "stay-path-zero-hops" ["EGSP-PATH", "EGSP-HOPS"],
  Target.mk ``step_path_adds_one_hop
    "step-path-successor-hop" ["EGSP-PATH", "EGSP-HOPS"],
  Target.mk ``map_preserves_hop_count
    "edge-subset-map-preserves-hops" ["EGSP-PATH", "EGSP-SUBSET"],
  Target.mk ``global_shortest_implies_shortest_in_sound_frontier
    "global-implies-frontier-shortest" ["EGSP-GLOBAL", "EGSP-FRONTIER"],
  Target.mk ``shortest_in_hop_complete_frontier_implies_global_shortest
    "complete-frontier-lifts-optimality" ["EGSP-COMPLETE", "EGSP-GLOBAL"],
  Target.mk ``frontier_minimum_alone_does_not_imply_global_minimum
    "frontier-minimum-insufficient" ["EGSP-FRONTIER", "EGSP-COUNTEREXAMPLE"],
  Target.mk ``authorized_edge_is_a_graph_edge
    "authorized-edge-soundness" ["EGSP-AUTH", "EGSP-EDGE"],
  Target.mk ``shortest_path_inherits_hop_budget_from_feasible_alternative
    "shortest-path-hop-feasibility" ["EGSP-BUDGET", "EGSP-SHORTEST"],
  Target.mk ``correct_hop_claim_is_valid
    "derived-hop-claim-valid" ["EGSP-CLAIM", "EGSP-HOPS"],
  Target.mk ``changed_hop_claim_is_invalid
    "changed-hop-claim-invalid" ["EGSP-CLAIM", "EGSP-FAIL-CLOSED"],
  Target.mk ``exact_remaining_cost_is_admissible
    "exact-heuristic-admissible" ["EGSP-HEURISTIC", "EGSP-ADMISSIBLE"],
  Target.mk ``overestimating_remaining_cost_is_not_admissible
    "overestimate-not-admissible" ["EGSP-HEURISTIC", "EGSP-COUNTEREXAMPLE"],
  Target.mk ``candidate_below_unseen_lower_bound_beats_every_unseen_path
    "unseen-lower-bound-certificate" ["EGSP-HEURISTIC", "EGSP-UNSEEN"],
  Target.mk ``complete_frontier_authorizes_global_unreachable_claim
    "complete-frontier-unreachable" ["EGSP-REACHABILITY", "EGSP-COMPLETE"],
  Target.mk ``truncated_frontier_cannot_authorize_global_unreachable_claim
    "truncated-frontier-not-unreachable" ["EGSP-REACHABILITY", "EGSP-TRUNCATED"]
]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchRouteEvidenceGraphShortestPathFrontierCompleteness"
    "ASPProof/SearchRouteEvidenceGraphShortestPathFrontierCompleteness.lean"
    targets

end ASPProof.Audit.SearchRouteEvidenceGraphShortestPathFrontierCompleteness

open ASPProof.Audit.SearchRouteEvidenceGraphShortestPathFrontierCompleteness

elab "writeSearchRouteEvidenceGraphShortestPathFrontierCompletenessAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-evidence-graph-shortest-path-frontier-completeness-audit-v1.json"
    auditJson
