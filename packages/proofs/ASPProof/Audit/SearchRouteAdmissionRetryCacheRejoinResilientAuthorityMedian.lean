import ASPProof.Audit.Core
import ASPProof.SearchRouteAdmissionRetryCacheRejoinResilientAuthorityMedian

namespace ASPProof.Audit

def writeReceipt
    (path : System.FilePath)
    (auditJson : Lean.Elab.TermElabM Lean.Json) :
    Lean.Elab.Command.CommandElabM Unit := do
  let audit ← Lean.Elab.Command.liftTermElabM auditJson
  IO.FS.writeFile path (audit.pretty ++ "\n")
  Lean.logInfo m!"wrote {path}"

end ASPProof.Audit

namespace ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinResilientAuthorityMedian

open ASPProof.Audit.Core
open ASPProof.SearchRouteAdmissionRetryCacheRejoinResilientAuthorityMedian

def targets : List Target := [
  Target.mk
    ``median_order_exists
    "three-authority-median-totality"
    ["RAM-ORDER", "RAM-MEDIAN"],
  Target.mk
    ``resilient_median_is_safe_and_live
    "one-byzantine-median-safety-liveness"
    ["RAM-COVERAGE", "RAM-MEDIAN"],
  Target.mk
    ``maximum_can_violate_live_upper_bound
    "maximum-liveness-counterexample"
    ["RAM-COUNTEREXAMPLE", "RAM-LIVENESS"],
  Target.mk
    ``minimum_can_violate_actual_fault_lower_bound
    "minimum-safety-counterexample"
    ["RAM-COUNTEREXAMPLE", "RAM-SAFETY"],
  Target.mk
    ``equivocation_cannot_fill_two_distinct_authority_slots
    "distinct-authority-equivocation-rejection"
    ["RAM-DISTINCT", "RAM-EQUIVOCATION"]
]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryCacheRejoinResilientAuthorityMedian"
    "ASPProof/SearchRouteAdmissionRetryCacheRejoinResilientAuthorityMedian.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinResilientAuthorityMedian
