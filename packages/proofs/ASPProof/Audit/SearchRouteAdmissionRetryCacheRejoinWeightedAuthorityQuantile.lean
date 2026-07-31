import ASPProof.Audit.Core
import ASPProof.SearchRouteAdmissionRetryCacheRejoinWeightedAuthorityQuantile

namespace ASPProof.Audit

def writeReceipt
    (path : System.FilePath)
    (auditJson : Lean.Elab.TermElabM Lean.Json) :
    Lean.Elab.Command.CommandElabM Unit := do
  let audit ← Lean.Elab.Command.liftTermElabM auditJson
  IO.FS.writeFile path (audit.pretty ++ "\n")
  Lean.logInfo m!"wrote {path}"

end ASPProof.Audit

namespace ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinWeightedAuthorityQuantile

open ASPProof.Audit.Core
open ASPProof.SearchRouteAdmissionRetryCacheRejoinWeightedAuthorityQuantile

def targets : List Target := [
  Target.mk
    ``weighted_quorum_places_canonical_cut_in_window
    "weighted-quorum-canonical-cut"
    ["WAQ-QUORUM", "WAQ-CUT"],
  Target.mk
    ``weighted_cut_certificate_is_safe_and_live
    "weighted-cut-safety-liveness"
    ["WAQ-CUT", "WAQ-SAFETY", "WAQ-LIVENESS"],
  Target.mk
    ``weighted_cut_window_implies_weighted_collected_quorum
    "weighted-collected-quorum-necessity"
    ["WAQ-CUT", "WAQ-QUORUM"],
  Target.mk
    ``voting_weight_budget_does_not_imply_authority_count_budget
    "weight-versus-count-budget-counterexample"
    ["WAQ-COUNTEREXAMPLE", "WAQ-IDENTITY"],
  Target.mk
    ``count_selected_value_can_violate_weighted_lower_bound
    "count-selection-weighted-safety-counterexample"
    ["WAQ-COUNTEREXAMPLE", "WAQ-SAFETY"],
  Target.mk
    ``unweighted_report_count_does_not_establish_weighted_quorum
    "count-quorum-weighted-quorum-counterexample"
    ["WAQ-COUNTEREXAMPLE", "WAQ-QUORUM"]
]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryCacheRejoinWeightedAuthorityQuantile"
    "ASPProof/SearchRouteAdmissionRetryCacheRejoinWeightedAuthorityQuantile.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinWeightedAuthorityQuantile
