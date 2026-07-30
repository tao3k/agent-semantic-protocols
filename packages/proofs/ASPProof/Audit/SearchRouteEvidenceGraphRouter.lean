import ASPProof.Audit.Core
import ASPProof.SearchRouteEvidenceGraphRouter

namespace ASPProof.Audit.SearchRouteEvidenceGraphRouter

open Lean
open ASPProof.Audit.Core

def targets : List Target :=
  [
    { name := ``_root_.SearchRouteEvidenceGraphRouter.certified_feasible_is_runtime_ready
      theoremFamily := "runtime-readiness"
      rfcClauseIds := [
        "ASP-RFC-10.05-EGR-CAPABILITY",
        "ASP-RFC-10.05-EGR-CERTIFIED"
      ] },
    { name := ``_root_.SearchRouteEvidenceGraphRouter.root_mismatch_rejects_candidate
      theoremFamily := "receipt-binding"
      rfcClauseIds := [
        "ASP-RFC-10.05-EGR-CERTIFIED",
        "ASP-RFC-10.05-EGR-SNAPSHOT"
      ] },
    { name := ``_root_.SearchRouteEvidenceGraphRouter.recoverable_candidate_is_not_runtime_ready
      theoremFamily := "recovery-exclusion"
      rfcClauseIds := [
        "ASP-RFC-10.05-EGR-CAPABILITY",
        "ASP-RFC-10.05-EGR-RECOVERY"
      ] },
    { name := ``_root_.SearchRouteEvidenceGraphRouter.incompatible_projection_rejects_candidate
      theoremFamily := "projection-compatibility"
      rfcClauseIds := [
        "ASP-RFC-10.05-EGR-CERTIFIED",
        "ASP-RFC-10.05-EGR-PROJECTION"
      ] },
    { name := ``_root_.SearchRouteEvidenceGraphRouter.graph_feasibility_does_not_imply_runtime_readiness
      theoremFamily := "static-feasibility-counterexample"
      rfcClauseIds := [
        "ASP-RFC-10.05-EGR-CAPABILITY",
        "ASP-RFC-10.05-EGR-STATIC-GAP"
      ] },
    { name := ``_root_.SearchRouteEvidenceGraphRouter.certified_selection_is_catalog_optimal
      theoremFamily := "catalog-optimality"
      rfcClauseIds := [
        "ASP-RFC-10.05-EGR-CATALOG",
        "ASP-RFC-10.05-EGR-CERTIFIED"
      ] },
    { name := ``_root_.SearchRouteEvidenceGraphRouter.complete_certified_selection_is_universe_optimal
      theoremFamily := "universe-optimality"
      rfcClauseIds := [
        "ASP-RFC-10.05-EGR-CATALOG",
        "ASP-RFC-10.05-EGR-GLOBAL"
      ] }
  ]

def auditJson : Elab.Term.TermElabM Json :=
  proofAuditJson
    "ASPProof.SearchRouteEvidenceGraphRouter"
    "packages/proofs/ASPProof/SearchRouteEvidenceGraphRouter.lean"
    targets

end ASPProof.Audit.SearchRouteEvidenceGraphRouter
