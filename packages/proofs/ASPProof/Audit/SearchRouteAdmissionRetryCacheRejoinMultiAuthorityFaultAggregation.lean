import ASPProof.Audit.Core
import ASPProof.SearchRouteAdmissionRetryCacheRejoinMultiAuthorityFaultAggregation

namespace ASPProof.Audit

def writeReceipt
    (path : System.FilePath)
    (auditJson : Lean.Elab.TermElabM Lean.Json) :
    Lean.Elab.Command.CommandElabM Unit := do
  let audit ← Lean.Elab.Command.liftTermElabM auditJson
  IO.FS.writeFile path (audit.pretty ++ "\n")
  Lean.logInfo m!"wrote {path}"

end ASPProof.Audit

namespace ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinMultiAuthorityFaultAggregation

open ASPProof.Audit.Core
open ASPProof.SearchRouteAdmissionRetryCacheRejoinMultiAuthorityFaultAggregation

def targets : List Target := [
  Target.mk
    ``candidate_bound_is_at_most_maximum
    "candidate-maximum-bound"
    ["MAFA-MAX", "MAFA-COVERAGE"],
  Target.mk
    ``conservative_witness_makes_maximum_safe
    "conservative-witness-safety"
    ["MAFA-WITNESS", "MAFA-MAX"],
  Target.mk
    ``adding_candidate_does_not_decrease_maximum
    "monotone-progressive-aggregation"
    ["MAFA-MONOTONE", "MAFA-LOOP"],
  Target.mk
    ``minimum_aggregation_can_understate_actual_fault_weight
    "unsafe-minimum-counterexample"
    ["MAFA-REJECT", "MAFA-WITNESS"],
  Target.mk
    ``partial_authority_search_can_pass_before_final_bound_fails
    "premature-stop-counterexample"
    ["MAFA-LOOP", "MAFA-REJECT"],
  Target.mk
    ``authentic_reports_without_conservative_witness_are_insufficient
    "authenticity-not-soundness"
    ["MAFA-AUTHENTIC", "MAFA-WITNESS"]
]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryCacheRejoinMultiAuthorityFaultAggregation"
    "ASPProof/SearchRouteAdmissionRetryCacheRejoinMultiAuthorityFaultAggregation.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinMultiAuthorityFaultAggregation
