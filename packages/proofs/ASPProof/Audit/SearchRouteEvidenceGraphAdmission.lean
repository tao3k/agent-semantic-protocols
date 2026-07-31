import ASPProof.Audit.Core
import ASPProof.SearchRouteEvidenceGraphAdmission

namespace ASPProof.Audit.SearchRouteEvidenceGraphAdmission

open Lean
open ASPProof.Audit.Core

def targets : List Target :=
  [
    { name :=
        ``SearchRouteEvidenceGraphAdmission.admitted_change_has_two_independent_beneficiaries
      theoremFamily := "independent-value"
      rfcClauseIds := ["ASP-RFC-10.05-EGA-VALUE"] },
    { name :=
        ``SearchRouteEvidenceGraphAdmission.admitted_change_beneficiary_owners_are_distinct
      theoremFamily := "independent-value"
      rfcClauseIds := [
        "ASP-RFC-10.05-EGA-VALUE",
        "ASP-RFC-10.05-EGA-POLICY"
      ] },
    { name :=
        ``SearchRouteEvidenceGraphAdmission.admitted_change_preserves_search_correctness
      theoremFamily := "search-correctness"
      rfcClauseIds := ["ASP-RFC-10.05-EGA-CORRECTNESS"] },
    { name :=
        ``SearchRouteEvidenceGraphAdmission.admitted_change_has_bounded_externality
      theoremFamily := "bounded-externality"
      rfcClauseIds := [
        "ASP-RFC-10.05-EGA-RUNTIME",
        "ASP-RFC-10.05-EGA-COMPLEXITY"
      ] },
    { name :=
        ``SearchRouteEvidenceGraphAdmission.admitted_change_is_isolated_and_deletable
      theoremFamily := "isolation-deletion"
      rfcClauseIds := [
        "ASP-RFC-10.05-EGA-ISOLATION",
        "ASP-RFC-10.05-EGA-DELETION"
      ] },
    { name :=
        ``SearchRouteEvidenceGraphAdmission.admitted_change_uses_authorized_policy
      theoremFamily := "policy-authority"
      rfcClauseIds := [
        "ASP-RFC-10.05-EGA-POLICY",
        "ASP-RFC-10.05-EGA-RUNTIME",
        "ASP-RFC-10.05-EGA-COMPLEXITY",
        "ASP-RFC-10.05-EGA-ISOLATION"
      ] },
    { name :=
        ``SearchRouteEvidenceGraphAdmission.admitted_change_has_authorized_measurement_coverage
      theoremFamily := "measurement-coverage"
      rfcClauseIds := [
        "ASP-RFC-10.05-EGA-COVERAGE",
        "ASP-RFC-10.05-EGA-MEASUREMENT",
        "ASP-RFC-10.05-EGA-POLICY"
      ] },
    { name :=
        ``SearchRouteEvidenceGraphAdmission.admitted_change_binds_authorized_evidence_verifier
      theoremFamily := "evidence-verifier-binding"
      rfcClauseIds := [
        "ASP-RFC-10.05-EGA-EVIDENCE",
        "ASP-RFC-10.05-EGA-CORRECTNESS",
        "ASP-RFC-10.05-EGA-POLICY"
      ] },
    { name :=
        ``SearchRouteEvidenceGraphAdmission.admitted_change_has_bounded_lifecycle
      theoremFamily := "bounded-lifecycle"
      rfcClauseIds := ["ASP-RFC-10.05-EGA-SUNSET"] },
    { name :=
        ``SearchRouteEvidenceGraphAdmission.silence_does_not_renew_expired_feature
      theoremFamily := "silence-sunset"
      rfcClauseIds := [
        "ASP-RFC-10.05-EGA-SUNSET",
        "ASP-RFC-10.05-EGA-NONIMPLICATION"
      ] },
    { name :=
        ``SearchRouteEvidenceGraphAdmission.unauthorized_receipt_does_not_renew_expired_feature
      theoremFamily := "renewal-authority"
      rfcClauseIds := [
        "ASP-RFC-10.05-EGA-SUNSET",
        "ASP-RFC-10.05-EGA-POLICY"
      ] },
    { name :=
        ``SearchRouteEvidenceGraphAdmission.authorized_timely_renewal_extends_lifecycle
      theoremFamily := "authorized-renewal"
      rfcClauseIds := [
        "ASP-RFC-10.05-EGA-SUNSET",
        "ASP-RFC-10.05-EGA-POLICY"
      ] },
    { name :=
        ``SearchRouteEvidenceGraphAdmission.fewer_graph_hops_do_not_imply_lower_agent_cost
      theoremFamily := "hop-counterexample"
      rfcClauseIds := [
        "ASP-RFC-10.05-EGA-RUNTIME",
        "ASP-RFC-10.05-EGA-NONIMPLICATION"
      ] },
    { name :=
        ``SearchRouteEvidenceGraphAdmission.one_owner_improvement_does_not_establish_shared_value
      theoremFamily := "single-owner-counterexample"
      rfcClauseIds := [
        "ASP-RFC-10.05-EGA-VALUE",
        "ASP-RFC-10.05-EGA-NONIMPLICATION"
      ] },
    { name :=
        ``SearchRouteEvidenceGraphAdmission.bounded_externality_does_not_imply_isolation
      theoremFamily := "isolation-counterexample"
      rfcClauseIds := [
        "ASP-RFC-10.05-EGA-RUNTIME",
        "ASP-RFC-10.05-EGA-ISOLATION",
        "ASP-RFC-10.05-EGA-NONIMPLICATION"
      ] },
    { name :=
        ``SearchRouteEvidenceGraphAdmission.unchanged_router_has_no_admission_value
      theoremFamily := "no-value-counterexample"
      rfcClauseIds := [
        "ASP-RFC-10.05-EGA-VALUE",
        "ASP-RFC-10.05-EGA-DELETION",
        "ASP-RFC-10.05-EGA-NONIMPLICATION"
      ] },
    { name :=
        ``SearchRouteEvidenceGraphAdmission.lower_cost_under_different_cache_context_is_not_certified
      theoremFamily := "measurement-context-counterexample"
      rfcClauseIds := [
        "ASP-RFC-10.05-EGA-MEASUREMENT",
        "ASP-RFC-10.05-EGA-VALUE",
        "ASP-RFC-10.05-EGA-NONIMPLICATION"
      ] },
    { name :=
        ``SearchRouteEvidenceGraphAdmission.better_point_estimate_does_not_imply_robust_improvement
      theoremFamily := "uncertainty-counterexample"
      rfcClauseIds := [
        "ASP-RFC-10.05-EGA-MEASUREMENT",
        "ASP-RFC-10.05-EGA-NONIMPLICATION"
      ] },
    { name :=
        ``SearchRouteEvidenceGraphAdmission.valid_returned_evidence_does_not_imply_required_evidence_completeness
      theoremFamily := "evidence-completeness-counterexample"
      rfcClauseIds := [
        "ASP-RFC-10.05-EGA-EVIDENCE",
        "ASP-RFC-10.05-EGA-CORRECTNESS",
        "ASP-RFC-10.05-EGA-NONIMPLICATION"
      ] },
    { name :=
        ``SearchRouteEvidenceGraphAdmission.deleting_router_code_does_not_delete_derived_search_state
      theoremFamily := "deletion-footprint-counterexample"
      rfcClauseIds := [
        "ASP-RFC-10.05-EGA-DELETION",
        "ASP-RFC-10.05-EGA-NONIMPLICATION"
      ] },
    { name :=
        ``SearchRouteEvidenceGraphAdmission.erasing_feature_causal_closure_restores_baseline_state
      theoremFamily := "deletion-causal-closure"
      rfcClauseIds := ["ASP-RFC-10.05-EGA-DELETION"] },
    { name :=
        ``SearchRouteEvidenceGraphAdmission.expired_silent_feature_requires_causal_closure_erasure
      theoremFamily := "sunset-deletion"
      rfcClauseIds := [
        "ASP-RFC-10.05-EGA-SUNSET",
        "ASP-RFC-10.05-EGA-DELETION"
      ] }
  ]

def auditJson : Elab.Term.TermElabM Json :=
  proofAuditJson
    "ASPProof.SearchRouteEvidenceGraphAdmission"
    "packages/proofs/ASPProof/SearchRouteEvidenceGraphAdmission.lean"
    targets

end ASPProof.Audit.SearchRouteEvidenceGraphAdmission
