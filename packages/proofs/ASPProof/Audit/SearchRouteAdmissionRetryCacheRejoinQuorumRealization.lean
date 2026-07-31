import ASPProof.Audit.Core
import ASPProof.SearchRouteAdmissionRetryCacheRejoinQuorumRealization

namespace ASPProof.Audit

def writeReceipt
    (path : System.FilePath)
    (auditJson : Lean.Elab.TermElabM Lean.Json) :
    Lean.Elab.Command.CommandElabM Unit := do
  let audit ← Lean.Elab.Command.liftTermElabM auditJson
  IO.FS.writeFile path (audit.pretty ++ "\n")
  Lean.logInfo m!"wrote {path}"

end ASPProof.Audit

namespace ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinQuorumRealization

open ASPProof.Audit.Core
open ASPProof.SearchRouteAdmissionRetryCacheRejoinQuorumRealization

def targets : List Target := [
  Target.mk
    ``accepted_certificate_realizes_weight_threshold
    "weighted-quorum-realization"
    ["CRQR-WEIGHT", "CRQR-ACCEPT"],
  Target.mk
    ``accepted_certificate_realizes_failure_domain_threshold
    "failure-domain-realization"
    ["CRQR-DOMAIN", "CRQR-ACCEPT"],
  Target.mk
    ``insufficient_voting_weight_rejects_certificate
    "insufficient-weight-rejection"
    ["CRQR-WEIGHT", "CRQR-REJECT"],
  Target.mk
    ``insufficient_failure_domain_coverage_rejects_certificate
    "insufficient-domain-rejection"
    ["CRQR-DOMAIN", "CRQR-REJECT"],
  Target.mk
    ``unknown_signer_rejects_certificate
    "membership-bounded-signers"
    ["CRQR-SIGNER", "CRQR-REJECT"],
  Target.mk
    ``duplicate_signer_rejects_certificate
    "duplicate-signer-rejection"
    ["CRQR-SIGNER", "CRQR-REJECT"],
  Target.mk
    ``canonical_membership_well_formedness_does_not_imply_feasibility
    "policy-feasibility-counterexample"
    ["CRQR-FEASIBLE", "CRQR-GAP"]
]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryCacheRejoinQuorumRealization"
    "ASPProof/SearchRouteAdmissionRetryCacheRejoinQuorumRealization.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinQuorumRealization
