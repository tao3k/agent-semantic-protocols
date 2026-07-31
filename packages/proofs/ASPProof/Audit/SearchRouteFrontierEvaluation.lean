import ASPProof.Audit.Core
import ASPProof.SearchRouteFrontierEvaluation

namespace ASPProof.Audit.SearchRouteFrontierEvaluation

open ASPProof.Audit.Core

def targets : List Target := [
  Target.mk
    `SearchRouteFrontierEvaluation.noWorse_trans
    "cost-order"
    ["ASP-RFC-10.05-EGF-COMPLETE"],
  Target.mk
    `SearchRouteFrontierEvaluation.noWorse_antisymm
    "cost-order"
    ["ASP-RFC-10.05-EGF-COMPLETE"],
  Target.mk
    `SearchRouteFrontierEvaluation.strictlyBetter_of_noWorse_of_strictlyBetter
    "dominance-preservation"
    ["ASP-RFC-10.05-EGF-COMPLETE", "ASP-RFC-10.05-EGF-OPTIMALITY"],
  Target.mk
    `SearchRouteFrontierEvaluation.complete_frontier_local_optimum_is_global
    "frontier-promotion"
    ["ASP-RFC-10.05-EGF-COMPLETE", "ASP-RFC-10.05-EGF-OPTIMALITY"],
  Target.mk
    `SearchRouteFrontierEvaluation.incomplete_frontier_can_hide_a_globally_better_route
    "incomplete-frontier-counterexample"
    ["ASP-RFC-10.05-EGF-NONIMPLICATION", "ASP-RFC-10.05-EGF-OPTIMALITY"],
  Target.mk
    `SearchRouteFrontierEvaluation.cheaper_but_evidence_incomplete_route_is_not_an_eligible_dominator
    "evidence-eligibility-counterexample"
    ["ASP-RFC-10.05-EGF-COMPLETE", "ASP-RFC-10.05-EGF-NONIMPLICATION"],
  Target.mk
    `SearchRouteFrontierEvaluation.certified_generalization_uses_disjoint_evaluation
    "evaluation-independence"
    ["ASP-RFC-10.05-EGF-SPLIT"],
  Target.mk
    `SearchRouteFrontierEvaluation.certified_generalization_binds_candidate_universe_identity
    "candidate-universe-commitment"
    ["ASP-RFC-10.05-EGF-COMMIT"],
  Target.mk
    `SearchRouteFrontierEvaluation.certified_generalization_binds_cache_identities
    "cache-identity-binding"
    ["ASP-RFC-10.05-EGF-CACHE", "ASP-RFC-10.05-EGF-COMMIT"],
  Target.mk
    `SearchRouteFrontierEvaluation.certified_generalization_compares_distinct_router_policies
    "router-policy-pair"
    ["ASP-RFC-10.05-EGF-COMMIT"],
  Target.mk
    `SearchRouteFrontierEvaluation.bound_generalization_uses_authorized_semantic_commitments
    "semantic-commitment-binding"
    ["ASP-RFC-10.05-EGF-COMMIT"],
  Target.mk
    `SearchRouteFrontierEvaluation.injective_content_addressing_is_non_equivocating
    "content-addressed-non-equivocation"
    ["ASP-RFC-10.05-EGF-COMMIT", "ASP-RFC-10.05-EGF-NON-EQUIVOCATION"],
  Target.mk
    `SearchRouteFrontierEvaluation.bound_generalization_verifiers_do_not_equivocate
    "bound-verifier-non-equivocation"
    ["ASP-RFC-10.05-EGF-NON-EQUIVOCATION"],
  Target.mk
    `SearchRouteFrontierEvaluation.verified_identity_has_cross_receipt_consistency
    "cross-receipt-consistency"
    ["ASP-RFC-10.05-EGF-NON-EQUIVOCATION"],
  Target.mk
    `SearchRouteFrontierEvaluation.equal_candidate_universe_identity_does_not_bind_candidate_semantics
    "opaque-identity-counterexample"
    ["ASP-RFC-10.05-EGF-COMMIT", "ASP-RFC-10.05-EGF-NONIMPLICATION"],
  Target.mk
    `SearchRouteFrontierEvaluation.permissive_verifier_accepts_conflicting_bindings
    "permissive-verifier-counterexample"
    ["ASP-RFC-10.05-EGF-NON-EQUIVOCATION", "ASP-RFC-10.05-EGF-NONIMPLICATION"],
  Target.mk
    `SearchRouteFrontierEvaluation.generation_no_older_trans
    "generation-monotonicity"
    ["ASP-RFC-10.05-EGF-GENERATION"],
  Target.mk
    `SearchRouteFrontierEvaluation.temporal_certificate_uses_current_generation
    "current-generation-binding"
    ["ASP-RFC-10.05-EGF-GENERATION"],
  Target.mk
    `SearchRouteFrontierEvaluation.temporal_certificate_rejects_consumed_receipts
    "anti-replay"
    ["ASP-RFC-10.05-EGF-ANTI-REPLAY"],
  Target.mk
    `SearchRouteFrontierEvaluation.semantic_non_equivocation_does_not_imply_freshness
    "freshness-counterexample"
    ["ASP-RFC-10.05-EGF-FRESHNESS", "ASP-RFC-10.05-EGF-NONIMPLICATION"],
  Target.mk
    `SearchRouteFrontierEvaluation.fresh_receipt_can_still_be_replayed
    "replay-counterexample"
    ["ASP-RFC-10.05-EGF-ANTI-REPLAY", "ASP-RFC-10.05-EGF-NONIMPLICATION"],
  Target.mk
    `SearchRouteFrontierEvaluation.same_run_identity_does_not_imply_same_generation
    "run-generation-counterexample"
    ["ASP-RFC-10.05-EGF-GENERATION", "ASP-RFC-10.05-EGF-NONIMPLICATION"],
  Target.mk
    `SearchRouteFrontierEvaluation.atomic_admission_marks_both_receipts_consumed
    "atomic-receipt-consumption"
    ["ASP-RFC-10.05-EGF-ANTI-REPLAY"],
  Target.mk
    `SearchRouteFrontierEvaluation.read_only_unconsumed_checks_do_not_consume_receipts
    "check-consume-counterexample"
    ["ASP-RFC-10.05-EGF-ANTI-REPLAY", "ASP-RFC-10.05-EGF-NONIMPLICATION"],
  Target.mk
    `SearchRouteFrontierEvaluation.training_success_on_the_evaluation_workload_is_not_generalization
    "evaluation-leakage-counterexample"
    ["ASP-RFC-10.05-EGF-NONIMPLICATION", "ASP-RFC-10.05-EGF-SPLIT"]
]

end ASPProof.Audit.SearchRouteFrontierEvaluation
