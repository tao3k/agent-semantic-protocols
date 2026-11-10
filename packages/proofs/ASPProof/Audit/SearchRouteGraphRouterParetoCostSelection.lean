-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteGraphRouterParetoCostSelection

namespace ASPProof.Audit

def writeReceipt
    (path : System.FilePath)
    (auditJson : Lean.Elab.TermElabM Lean.Json) :
    Lean.Elab.Command.CommandElabM Unit := do
  let audit ← Lean.Elab.Command.liftTermElabM auditJson
  IO.FS.writeFile path (audit.pretty ++ "\n")
  Lean.logInfo m!"wrote {path}"

end ASPProof.Audit

namespace ASPProof.Audit.SearchRouteGraphRouterParetoCostSelection

open ASPProof.Audit.Core
open ASPProof.SearchRouteGraphRouterParetoCostSelection

def targets : List Target := [
  Target.mk ``no_worse_is_reflexive
    "component-order-reflexive" ["GRPC-NO-WORSE", "GRPC-ORDER"],
  Target.mk ``no_worse_is_transitive
    "component-order-transitive" ["GRPC-NO-WORSE", "GRPC-ORDER"],
  Target.mk ``strict_dominance_implies_no_worse
    "strict-dominance-component-bound" ["GRPC-PARETO", "GRPC-DOMINANCE"],
  Target.mk ``strict_dominance_is_irreflexive
    "strict-dominance-irreflexive" ["GRPC-PARETO", "GRPC-ORDER"],
  Target.mk ``no_worse_route_preserves_feasibility
    "dominance-preserves-feasibility" ["GRPC-FEASIBLE", "GRPC-PARETO"],
  Target.mk ``fewer_graph_hops_is_hop_first_lex_better
    "hop-first-lex-head" ["GRPC-LEX", "GRPC-HOPS"],
  Target.mk ``hop_first_lex_order_requires_prior_feasibility
    "lex-needs-hard-caps" ["GRPC-LEX", "GRPC-COUNTEREXAMPLE"],
  Target.mk ``equal_unit_weight_score_can_hide_token_tradeoff
    "scalar-score-hides-token-tradeoff" ["GRPC-SCALAR", "GRPC-TOKENS"],
  Target.mk ``equal_unit_weight_routes_can_be_pareto_incomparable
    "scalar-equality-not-pareto-equivalence" ["GRPC-SCALAR", "GRPC-PARETO"],
  Target.mk ``combined_cache_work_can_hide_cache_direction
    "cache-aggregate-hides-direction" ["GRPC-CACHE", "GRPC-AGGREGATE"],
  Target.mk ``equal_combined_cache_work_can_be_pareto_incomparable
    "cache-aggregate-not-pareto-equivalence" ["GRPC-CACHE", "GRPC-PARETO"],
  Target.mk ``search_reuse_does_not_imply_model_prefix_reuse
    "search-reuse-not-model-reuse" ["GRPC-SEARCH-CACHE", "GRPC-MODEL-CACHE"],
  Target.mk ``model_prefix_reuse_does_not_imply_search_reuse
    "model-reuse-not-search-reuse" ["GRPC-MODEL-CACHE", "GRPC-SEARCH-CACHE"],
  Target.mk ``unchanged_route_evaluation_identity_is_comparable
    "evaluation-identity-reflexive" ["GRPC-IDENTITY", "GRPC-COMPARISON"],
  Target.mk ``changed_evaluation_snapshot_invalidates_comparison
    "snapshot-invalidates-route-comparison" ["GRPC-IDENTITY", "GRPC-SNAPSHOT"],
  Target.mk ``changed_evaluation_model_invalidates_comparison
    "model-invalidates-route-comparison" ["GRPC-IDENTITY", "GRPC-MODEL"]
]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchRouteGraphRouterParetoCostSelection"
    "ASPProof/SearchRouteGraphRouterParetoCostSelection.lean"
    targets

end ASPProof.Audit.SearchRouteGraphRouterParetoCostSelection

open ASPProof.Audit.SearchRouteGraphRouterParetoCostSelection

elab "writeSearchRouteGraphRouterParetoCostSelectionAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-graph-router-pareto-cost-selection-audit-v1.json"
    auditJson
