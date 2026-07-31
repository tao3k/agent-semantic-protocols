import ASPProof.Audit.Core
import ASPProof.SearchRouteAdmissionRetryCacheRejoinJointDecisionFusion

namespace ASPProof.Audit

def writeReceipt
    (path : System.FilePath)
    (auditJson : Lean.Elab.TermElabM Lean.Json) :
    Lean.Elab.Command.CommandElabM Unit := do
  let audit ← Lean.Elab.Command.liftTermElabM auditJson
  IO.FS.writeFile path (audit.pretty ++ "\n")
  Lean.logInfo m!"wrote {path}"

end ASPProof.Audit

namespace ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinJointDecisionFusion

open ASPProof.Audit.Core
open ASPProof.SearchRouteAdmissionRetryCacheRejoinJointDecisionFusion

def targets : List Target := [
  Target.mk
    ``conservative_maximum_exists
    "joint-maximum-totality"
    ["JDF-MAX", "JDF-TOTAL"],
  Target.mk
    ``conservative_maximum_is_safe_and_live
    "joint-maximum-interval-preservation"
    ["JDF-MAX", "JDF-SAFETY", "JDF-LIVENESS"],
  Target.mk
    ``conservative_maximum_dominates_both_inputs
    "joint-maximum-conservative-dominance"
    ["JDF-MAX", "JDF-DOMINANCE"],
  Target.mk
    ``interval_membership_alone_does_not_imply_conservative_dominance
    "interval-without-dominance-counterexample"
    ["JDF-COUNTEREXAMPLE", "JDF-DOMINANCE"],
  Target.mk
    ``missing_new_verification_blocks_joint_decision
    "missing-new-input-rejection"
    ["JDF-VERIFICATION", "JDF-NEW"],
  Target.mk
    ``missing_old_verification_blocks_joint_decision
    "missing-old-input-rejection"
    ["JDF-VERIFICATION", "JDF-OLD"],
  Target.mk
    ``joint_decision_compatibility_binds_both_inputs
    "joint-input-identity-binding"
    ["JDF-IDENTITY", "JDF-RECEIPT"],
  Target.mk
    ``changed_old_input_invalidates_joint_decision
    "old-input-cache-invalidation"
    ["JDF-CACHE", "JDF-OLD"],
  Target.mk
    ``changed_new_input_invalidates_joint_decision
    "new-input-cache-invalidation"
    ["JDF-CACHE", "JDF-NEW"],
  Target.mk
    ``same_fused_value_does_not_imply_joint_compatibility
    "same-result-identity-separation"
    ["JDF-COUNTEREXAMPLE", "JDF-IDENTITY"],
  Target.mk
    ``lane_swap_with_distinct_receipts_changes_joint_identity
    "lane-role-identity-separation"
    ["JDF-LANE", "JDF-IDENTITY"]
]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryCacheRejoinJointDecisionFusion"
    "ASPProof/SearchRouteAdmissionRetryCacheRejoinJointDecisionFusion.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinJointDecisionFusion
