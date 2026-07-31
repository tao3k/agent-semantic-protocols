import ASPProof.Audit.Core
import ASPProof.SearchRouteAdmissionRetryCacheRejoinGeneralAuthorityOrderStatistic

namespace ASPProof.Audit

def writeReceipt
    (path : System.FilePath)
    (auditJson : Lean.Elab.TermElabM Lean.Json) :
    Lean.Elab.Command.CommandElabM Unit := do
  let audit ← Lean.Elab.Command.liftTermElabM auditJson
  IO.FS.writeFile path (audit.pretty ++ "\n")
  Lean.logInfo m!"wrote {path}"

end ASPProof.Audit

namespace ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinGeneralAuthorityOrderStatistic

open ASPProof.Audit.Core
open ASPProof.SearchRouteAdmissionRetryCacheRejoinGeneralAuthorityOrderStatistic

def targets : List Target := [
  Target.mk
    ``quorum_places_fault_rank_interior
    "quorum-fault-rank-window"
    ["GAOS-QUORUM", "GAOS-RANK"],
  Target.mk
    ``rank_certified_order_statistic_is_safe_and_live
    "rank-certificate-safety-liveness"
    ["GAOS-RANK", "GAOS-SAFETY", "GAOS-LIVENESS"],
  Target.mk
    ``rank_window_implies_collected_quorum
    "collected-report-quorum-necessity"
    ["GAOS-COLLECTION", "GAOS-QUORUM"],
  Target.mk
    ``configured_quorum_does_not_cover_partial_collection
    "configured-versus-collected-counterexample"
    ["GAOS-COUNTEREXAMPLE", "GAOS-COLLECTION"],
  Target.mk
    ``rankless_selector_can_violate_actual_fault_lower_bound
    "rankless-lower-bound-counterexample"
    ["GAOS-COUNTEREXAMPLE", "GAOS-SAFETY"],
  Target.mk
    ``rankless_selector_can_violate_live_upper_bound
    "rankless-upper-bound-counterexample"
    ["GAOS-COUNTEREXAMPLE", "GAOS-LIVENESS"]
]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryCacheRejoinGeneralAuthorityOrderStatistic"
    "ASPProof/SearchRouteAdmissionRetryCacheRejoinGeneralAuthorityOrderStatistic.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinGeneralAuthorityOrderStatistic
