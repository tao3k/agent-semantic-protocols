import ASPProof.Audit.Receipt
import ASPProof.SearchRouteAdmissionRetryCacheRejoinJointConfiguration

namespace ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinJointConfiguration

open ASPProof.Audit.Core
open ASPProof.SearchRouteAdmissionRetryCacheRejoinJointConfiguration

def targets : List Target :=
  [ Target.mk ``old_certificate_and_joint_certificate_same_slot_agree
      "old-joint-history-bridge"
      [ "ASP-RFC-10.05-CRJC-OLD", "ASP-RFC-10.05-CRJC-VOTE" ]
  , Target.mk ``new_certificate_and_joint_certificate_same_slot_agree
      "new-joint-history-bridge"
      [ "ASP-RFC-10.05-CRJC-NEW", "ASP-RFC-10.05-CRJC-VOTE" ]
  , Target.mk ``missing_new_quorum_cannot_finalize_reconfiguration
      "missing-new-quorum-counterexample" [ "ASP-RFC-10.05-CRJC-JOINT" ]
  , Target.mk ``missing_old_quorum_cannot_finalize_reconfiguration
      "missing-old-quorum-counterexample" [ "ASP-RFC-10.05-CRJC-JOINT" ]
  ]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryCacheRejoinJointConfiguration"
    "ASPProof/SearchRouteAdmissionRetryCacheRejoinJointConfiguration.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinJointConfiguration
