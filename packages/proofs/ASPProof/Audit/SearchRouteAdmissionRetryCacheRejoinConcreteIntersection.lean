import ASPProof.Audit.Core
import ASPProof.SearchRouteAdmissionRetryCacheRejoinConcreteIntersection

namespace ASPProof.Audit

def writeReceipt
    (path : System.FilePath)
    (auditJson : Lean.Elab.TermElabM Lean.Json) :
    Lean.Elab.Command.CommandElabM Unit := do
  let audit ← Lean.Elab.Command.liftTermElabM auditJson
  IO.FS.writeFile path (audit.pretty ++ "\n")
  Lean.logInfo m!"wrote {path}"

end ASPProof.Audit

namespace ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinConcreteIntersection

open ASPProof.Audit.Core
open ASPProof.SearchRouteAdmissionRetryCacheRejoinConcreteIntersection

def targets : List Target := [
  Target.mk
    ``signer_selection_ignores_duplicate_head
    "boolean-signer-deduplication"
    ["CRCI-SELECT", "CRCI-DUPLICATE"],
  Target.mk
    ``projected_weight_ignores_duplicate_signer_head
    "duplicate-safe-realized-weight"
    ["CRCI-PROJECTION", "CRCI-DUPLICATE"],
  Target.mk
    ``intersection_weight_ignores_duplicate_left_signer_head
    "duplicate-safe-intersection-weight"
    ["CRCI-INTERSECTION", "CRCI-DUPLICATE"],
  Target.mk
    ``intersection_weight_is_bounded_by_total
    "intersection-total-bound"
    ["CRCI-INTERSECTION", "CRCI-BOUND"],
  Target.mk
    ``concrete_intersection_satisfies_inclusion_exclusion
    "finite-membership-inclusion-exclusion"
    ["CRCI-ACCOUNTING", "CRCI-BOUND"],
  Target.mk
    ``concrete_pair_accounting_requires_no_external_bound
    "constructible-pair-accounting"
    ["CRCI-ACCOUNTING", "CRCI-ROUTER"]
]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryCacheRejoinConcreteIntersection"
    "ASPProof/SearchRouteAdmissionRetryCacheRejoinConcreteIntersection.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinConcreteIntersection
