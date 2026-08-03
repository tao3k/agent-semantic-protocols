import ASPProof.Audit.Core
import ASPProof.SearchProjectionRenderInterface

namespace ASPProof.Audit

def writeSearchProjectionRenderInterfaceReceipt
    (path : System.FilePath)
    (auditJson : Lean.Elab.TermElabM Lean.Json) :
    Lean.Elab.Command.CommandElabM Unit := do
  let audit ← Lean.Elab.Command.liftTermElabM auditJson
  IO.FS.writeFile path (audit.pretty ++ "\n")
  Lean.logInfo m!"wrote {path}"

end ASPProof.Audit

namespace ASPProof.Audit.SearchProjectionRenderInterface

open ASPProof.Audit.Core
open ASPProof.SearchProjectionRenderInterface

def targets : List Target := [
  Target.mk ``control_grammar_rejects_mutation_and_commands
    "control-grammar-read-only" ["ASP-RFC-SPRI-CONTROL", "ASP-RFC-SPRI-AUTHORITY"],
  Target.mk ``gql_core_is_read_only_and_bounded
    "gql-core-read-only" ["ASP-RFC-SPRI-GQL", "ASP-RFC-SPRI-READ-ONLY"],
  Target.mk ``logic_query_cannot_define_rules_facts_or_host_code
    "logic-query-closed" ["ASP-RFC-SPRI-LOGIC", "ASP-RFC-SPRI-ASCENT"],
  Target.mk ``typed_relation_abi_is_well_formed
    "typed-relation-abi" ["ASP-RFC-SPRI-RELATION-ABI", "ASP-RFC-SPRI-GQL"],
  Target.mk ``relation_abi_version_drift_is_rejected
    "relation-abi-version-rejection" ["ASP-RFC-SPRI-RELATION-ABI", "ASP-RFC-SPRI-DRIFT"],
  Target.mk ``registered_derived_predicate_is_admitted
    "registered-derived-predicate" ["ASP-RFC-SPRI-ASCENT", "ASP-RFC-SPRI-RULESET"],
  Target.mk ``unregistered_derived_predicate_is_rejected
    "unregistered-derived-rejection" ["ASP-RFC-SPRI-ASCENT", "ASP-RFC-SPRI-RULESET"],
  Target.mk ``graph_turbo_score_cannot_mint_evidence_authority
    "graph-turbo-nonauthority" ["ASP-RFC-SPRI-TURBO", "ASP-RFC-SPRI-AUTHORITY"],
  Target.mk ``progressive_frontier_exposes_ten_candidates
    "frontier-exposure-budget" ["ASP-RFC-SPRI-FRONTIER", "ASP-RFC-SPRI-EXPOSURE"],
  Target.mk ``model_may_select_three_of_ten_candidates
    "bounded-multi-selection" ["ASP-RFC-SPRI-FRONTIER", "ASP-RFC-SPRI-SELECTION"],
  Target.mk ``model_cannot_exceed_selection_budget
    "selection-budget-rejection" ["ASP-RFC-SPRI-SELECTION", "ASP-RFC-SPRI-BOUND"],
  Target.mk ``model_cannot_select_an_unexposed_candidate
    "unexposed-selection-rejection" ["ASP-RFC-SPRI-SELECTION", "ASP-RFC-SPRI-AUTHORITY"],
  Target.mk ``bounded_first_page_does_not_imply_witness_visibility
    "first-page-completeness-counterexample" ["ASP-RFC-SPRI-COUNTEREXAMPLE", "ASP-RFC-SPRI-FRONTIER"],
  Target.mk ``fair_second_page_exposes_the_eleventh_witness
    "progressive-witness-recovery" ["ASP-RFC-SPRI-PROGRESSIVE", "ASP-RFC-SPRI-CONTINUATION"],
  Target.mk ``progressive_resume_monotonically_reveals_and_decreases_remaining
    "progressive-monotone-disclosure" ["ASP-RFC-SPRI-PROGRESSIVE", "ASP-RFC-SPRI-CONTINUATION"],
  Target.mk ``hidden_evidence_requires_an_omission_certificate
    "omission-certificate-required" ["ASP-RFC-SPRI-OMISSION", "ASP-RFC-SPRI-PROGRESSIVE"],
  Target.mk ``stale_progressive_continuation_is_rejected
    "progressive-stale-continuation" ["ASP-RFC-SPRI-CONTINUATION", "ASP-RFC-SPRI-PROGRESSIVE"],
  Target.mk ``internal_json_is_not_an_agent_encoding
    "json-not-agent-surface" ["ASP-RFC-SPRI-POLYGLOT", "ASP-RFC-SPRI-JSON"],
  Target.mk ``bounded_typed_polyglot_projection_is_admitted
    "bounded-polyglot-positive" ["ASP-RFC-SPRI-POLYGLOT", "ASP-RFC-SPRI-BOUND"],
  Target.mk ``projection_over_token_budget_is_rejected
    "projection-token-budget" ["ASP-RFC-SPRI-ENCODING", "ASP-RFC-SPRI-BOUND"],
  Target.mk ``truncation_cannot_remove_protected_identity
    "protected-identity-truncation" ["ASP-RFC-SPRI-IDENTITY", "ASP-RFC-SPRI-TRUNCATION"],
  Target.mk ``example_new_search_replacement_is_admitted
    "replacement-positive" ["ASP-RFC-SPRI-REFINEMENT", "ASP-RFC-SPRI-COST"],
  Target.mk ``lower_cost_cannot_replace_semantic_equivalence
    "cheap-unsound-replacement-rejection" ["ASP-RFC-SPRI-REFINEMENT", "ASP-RFC-SPRI-SOUNDNESS"],
  Target.mk ``machine_replacement_requires_semantic_refinement
    "parametric-machine-refinement" ["ASP-RFC-SPRI-REFINEMENT", "ASP-RFC-SPRI-WORKLOAD"],
  Target.mk ``machine_replacement_requires_declared_assumptions
    "replacement-assumption-admission" ["ASP-RFC-SPRI-ASSUMPTIONS", "ASP-RFC-SPRI-REFINEMENT"],
  Target.mk ``machine_replacement_preserves_evidence_digest
    "parametric-evidence-preservation" ["ASP-RFC-SPRI-EVIDENCE", "ASP-RFC-SPRI-REFINEMENT"],
  Target.mk ``missing_fair_continuation_blocks_replacement_qualification
    "fair-continuation-required" ["ASP-RFC-SPRI-CONTINUATION", "ASP-RFC-SPRI-ASSUMPTIONS"],
  Target.mk ``complete_replacement_certificate_is_admitted
    "replacement-certificate-positive" ["ASP-RFC-SPRI-CERTIFICATE", "ASP-RFC-SPRI-CUTOVER"],
  Target.mk ``runtime_digest_drift_blocks_replacement_certificate
    "replacement-runtime-drift-rejection" ["ASP-RFC-SPRI-CERTIFICATE", "ASP-RFC-SPRI-RUNTIME"],
  Target.mk ``one_benchmark_slice_cannot_qualify_replacement
    "independent-benchmark-slices-required" ["ASP-RFC-SPRI-CERTIFICATE", "ASP-RFC-SPRI-EMPIRICAL"],
  Target.mk ``ten_candidate_projection_has_a_linear_token_bound
    "linear-projection-token-bound" ["ASP-RFC-SPRI-TOKEN", "ASP-RFC-SPRI-BOUND"],
  Target.mk ``three_selected_branches_bound_next_expansion
    "selected-branch-expansion-bound" ["ASP-RFC-SPRI-SELECTION", "ASP-RFC-SPRI-GRAPH-COST"],
  Target.mk ``encoder_version_is_absent_from_search_result_cache_identity
    "search-cache-encoder-independence" ["ASP-RFC-SPRI-SEARCH-CACHE", "ASP-RFC-SPRI-ENCODING"],
  Target.mk ``different_encoder_versions_are_not_byte_compatible
    "encoder-byte-identity" ["ASP-RFC-SPRI-BYTE-CACHE", "ASP-RFC-SPRI-ENCODING"],
  Target.mk ``compact_and_visualization_adapters_cannot_mint_authority
    "adapter-nonauthority" ["ASP-RFC-SPRI-AUTHORITY", "ASP-RFC-SPRI-VISUALIZATION"]
]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchProjectionRenderInterface"
    "ASPProof/SearchProjectionRenderInterface.lean"
    targets

end ASPProof.Audit.SearchProjectionRenderInterface

open ASPProof.Audit.SearchProjectionRenderInterface

elab "writeSearchProjectionRenderInterfaceAudit" : command =>
  ASPProof.Audit.writeSearchProjectionRenderInterfaceReceipt
    "receipts/search-projection-render-interface-audit-v1.json"
    auditJson
