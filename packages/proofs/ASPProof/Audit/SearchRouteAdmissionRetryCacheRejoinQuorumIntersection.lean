import ASPProof.Audit.Core
import ASPProof.SearchRouteAdmissionRetryCacheRejoinQuorumIntersection

namespace ASPProof.Audit

def writeReceipt
    (path : System.FilePath)
    (auditJson : Lean.Elab.TermElabM Lean.Json) :
    Lean.Elab.Command.CommandElabM Unit := do
  let audit ← Lean.Elab.Command.liftTermElabM auditJson
  IO.FS.writeFile path (audit.pretty ++ "\n")
  Lean.logInfo m!"wrote {path}"

end ASPProof.Audit

namespace ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinQuorumIntersection

open ASPProof.Audit.Core
open ASPProof.SearchRouteAdmissionRetryCacheRejoinQuorumIntersection

def targets : List Target := [
  Target.mk
    ``accepted_certificates_establish_overlap_lower_bound
    "accepted-pair-overlap-lower-bound"
    ["CRQI-ACCOUNTING", "CRQI-PAIR"],
  Target.mk
    ``threshold_over_fault_bound_forces_honest_overlap
    "fault-bounded-honest-overlap"
    ["CRQI-HONEST", "CRQI-PAIR"],
  Target.mk
    ``strict_weight_majority_forces_positive_overlap
    "strict-majority-positive-overlap"
    ["CRQI-MAJORITY", "CRQI-PAIR"],
  Target.mk
    ``half_total_threshold_allows_disjoint_realized_quorums
    "half-threshold-disjoint-counterexample"
    ["CRQI-COUNTEREXAMPLE", "CRQI-MAJORITY"],
  Target.mk
    ``strict_majority_overlap_can_be_entirely_faulty
    "all-faulty-overlap-counterexample"
    ["CRQI-COUNTEREXAMPLE", "CRQI-HONEST"]
]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryCacheRejoinQuorumIntersection"
    "ASPProof/SearchRouteAdmissionRetryCacheRejoinQuorumIntersection.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinQuorumIntersection
