-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.GQLCoreSciPyResidentGraphTurbo

namespace ASPProof.Audit

def writeGQLCoreSciPyResidentGraphTurboReceipt
    (path : System.FilePath)
    (auditJson : Lean.Elab.TermElabM Lean.Json) :
    Lean.Elab.Command.CommandElabM Unit := do
  let audit ← Lean.Elab.Command.liftTermElabM auditJson
  IO.FS.writeFile path (audit.pretty ++ "\n")
  Lean.logInfo m!"wrote {path}"

end ASPProof.Audit

namespace ASPProof.Audit.GQLCoreSciPyResidentGraphTurbo

open ASPProof.Audit.Core
open ASPProof.GQLCoreSciPyResidentGraphTurbo

def targets : List Target := [
  Target.mk ``python_graph_outage_cannot_block_base_generation_commit
    "python-graph-outage-nonblocking" ["GSRGT-BASE-GENERATION", "GSRGT-PYTHON"],
  Target.mk ``python_graph_availability_does_not_change_base_generation_identity
    "python-graph-identity-noninterference" ["GSRGT-BASE-GENERATION", "GSRGT-PYTHON"],
  Target.mk ``python_proposal_cannot_mint_evidence_authority
    "python-proposal-nonauthority" ["GSRGT-AUTHORITY", "GSRGT-PYTHON"],
  Target.mk ``exact_graph_session_identity_is_admitted
    "exact-session-positive" ["GSRGT-SESSION", "GSRGT-CONTINUATION"],
  Target.mk ``cross_session_substitution_is_rejected
    "cross-session-rejection" ["GSRGT-SESSION", "GSRGT-CONTINUATION"],
  Target.mk ``valid_deserialized_hop_accounting_is_admitted
    "decoded-hop-positive" ["GSRGT-HOPS", "GSRGT-DESERIALIZATION"],
  Target.mk ``executed_hops_above_semantic_hops_are_rejected
    "decoded-hop-invariant" ["GSRGT-HOPS", "GSRGT-DESERIALIZATION"],
  Target.mk ``cache_resume_preserves_semantic_route_and_may_reduce_realization
    "cache-resume-semantic-invariance" ["GSRGT-CACHE", "GSRGT-CONTINUATION"],
  Target.mk ``cache_resume_cannot_rewrite_semantic_hops
    "cache-semantic-hop-counterexample" ["GSRGT-CACHE", "GSRGT-HOPS"],
  Target.mk ``equal_combined_cache_work_can_hide_orthogonal_cache_lanes
    "cache-aggregate-counterexample" ["GSRGT-SEARCH-CACHE", "GSRGT-MODEL-CACHE"],
  Target.mk ``search_cache_reuse_does_not_imply_model_prefix_reuse
    "search-cache-orthogonality" ["GSRGT-SEARCH-CACHE", "GSRGT-MODEL-CACHE"],
  Target.mk ``model_prefix_reuse_does_not_imply_search_cache_reuse
    "model-cache-orthogonality" ["GSRGT-SEARCH-CACHE", "GSRGT-MODEL-CACHE"],
  Target.mk ``bounded_ascent_receipt_is_snapshot_scoped_and_grounded
    "bounded-ascent-admission" ["GSRGT-ASCENT", "GSRGT-SNAPSHOT"],
  Target.mk ``recomputed_ascent_closure_is_grounded_complete_and_canonical
    "verified-fixed-point-positive" ["GSRGT-ASCENT", "GSRGT-FIXED-POINT"],
  Target.mk ``budget_only_admission_can_accept_a_non_fixed_point_receipt
    "budget-only-fixed-point-counterexample" ["GSRGT-ASCENT", "GSRGT-FIXED-POINT"],
  Target.mk ``verified_admission_rejects_a_non_fixed_point_receipt
    "verified-fixed-point-rejection" ["GSRGT-ASCENT", "GSRGT-FIXED-POINT"],
  Target.mk ``nonempty_digest_check_does_not_establish_canonical_order
    "canonical-digest-counterexample" ["GSRGT-ASCENT", "GSRGT-DETERMINISM"],
  Target.mk ``scalar_rule_firing_count_does_not_ground_a_derivation
    "scalar-rule-count-counterexample" ["GSRGT-ASCENT", "GSRGT-DERIVATION"],
  Target.mk ``model_injected_ascent_ruleset_is_rejected
    "trusted-ruleset-rejection" ["GSRGT-ASCENT", "GSRGT-RULESET"],
  Target.mk ``ascent_rule_firing_budget_is_independent_and_fail_closed
    "rule-firing-budget" ["GSRGT-ASCENT", "GSRGT-BUDGET"],
  Target.mk ``ascent_derived_fact_budget_is_independent_and_fail_closed
    "derived-fact-budget" ["GSRGT-ASCENT", "GSRGT-BUDGET"],
  Target.mk ``verified_graph_delta_can_admit_bounded_closure_output
    "graph-delta-refinement" ["GSRGT-GRAPH-DELTA", "GSRGT-ASCENT"],
  Target.mk ``ascent_receipt_alone_cannot_mutate_evidence_graph
    "closure-nonauthority" ["GSRGT-AUTHORITY", "GSRGT-ASCENT"],
  Target.mk ``python_proposal_alone_cannot_mutate_evidence_graph
    "proposal-nonauthority" ["GSRGT-AUTHORITY", "GSRGT-PYTHON"],
  Target.mk ``router_bounded_gql_path_is_admitted
    "bounded-gql-positive" ["GSRGT-GQL", "GSRGT-HOPS"],
  Target.mk ``unbounded_gql_path_is_fail_closed
    "unbounded-gql-rejection" ["GSRGT-GQL", "GSRGT-HOPS"],
  Target.mk ``pareto_improvement_preserves_hard_budget_feasibility
    "pareto-feasibility" ["GSRGT-PARETO", "GSRGT-BUDGET"],
  Target.mk ``shortest_hop_order_requires_prior_token_and_round_feasibility
    "hop-first-needs-feasibility" ["GSRGT-PARETO", "GSRGT-TOKENS"],
  Target.mk ``one_process_can_serve_distinct_isolated_graph_sessions
    "process-session-separation" ["GSRGT-PROCESS", "GSRGT-SESSION"],
  Target.mk ``search_projection_cache_identity_is_exact
    "projection-cache-positive" ["GSRGT-SEARCH-CACHE", "GSRGT-SNAPSHOT"],
  Target.mk ``model_prefix_cache_identity_is_exact
    "model-cache-positive" ["GSRGT-MODEL-CACHE", "GSRGT-CACHE"],
  Target.mk ``stale_snapshot_invalidates_search_projection_cache
    "projection-cache-snapshot-fence" ["GSRGT-SEARCH-CACHE", "GSRGT-SNAPSHOT"],
  Target.mk ``renderer_bytes_are_outside_search_and_model_cache_identity
    "renderer-identity-noninterference" ["GSRGT-RENDERER", "GSRGT-CACHE"]
]

def canonicalTargets : List Target :=
  targets.map fun target =>
    { target with
      rfcClauseIds := target.rfcClauseIds.map fun clauseId =>
        "ASP-RFC-" ++ clauseId }

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.GQLCoreSciPyResidentGraphTurbo"
    "ASPProof/GQLCoreSciPyResidentGraphTurbo.lean"
    canonicalTargets

end ASPProof.Audit.GQLCoreSciPyResidentGraphTurbo

open ASPProof.Audit.GQLCoreSciPyResidentGraphTurbo

elab "writeGQLCoreSciPyResidentGraphTurboAudit" : command =>
  ASPProof.Audit.writeGQLCoreSciPyResidentGraphTurboReceipt
    "receipts/gql-core-scipy-resident-graph-turbo-audit-v1.json"
    auditJson
