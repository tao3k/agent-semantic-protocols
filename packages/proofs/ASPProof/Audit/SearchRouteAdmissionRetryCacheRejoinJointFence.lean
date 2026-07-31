import ASPProof.Audit.Receipt
import ASPProof.SearchRouteAdmissionRetryCacheRejoinJointFence

namespace ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinJointFence

open ASPProof.Audit.Core
open ASPProof.SearchRouteAdmissionRetryCacheRejoinJointFence

def targets : List Target :=
  [ Target.mk
      ``committable_joint_acknowledgement_matches_all_heads
      "composite-fence-gate"
      [ "ASP-RFC-10.05-CRJF-CONTENT"
      , "ASP-RFC-10.05-CRJF-GATE"
      ]
  , Target.mk ``stale_joint_fence_cannot_commit
      "stale-fence-counterexample" [ "ASP-RFC-10.05-CRJF-MISMATCH" ]
  , Target.mk ``mismatched_admission_head_cannot_commit_joint_acknowledgement
      "admission-head-counterexample" [ "ASP-RFC-10.05-CRJF-MISMATCH" ]
  , Target.mk ``mismatched_policy_head_cannot_commit_joint_acknowledgement
      "policy-head-counterexample" [ "ASP-RFC-10.05-CRJF-MISMATCH" ]
  , Target.mk ``mismatched_policy_digest_cannot_commit_joint_acknowledgement
      "policy-digest-counterexample" [ "ASP-RFC-10.05-CRJF-MISMATCH" ]
  , Target.mk ``joint_commit_advances_sequence_and_closes_obligation
      "joint-commit-successor" [ "ASP-RFC-10.05-CRJF-COMMIT" ]
  , Target.mk ``same_snapshot_second_joint_acknowledgement_loses
      "joint-one-winner" [ "ASP-RFC-10.05-CRJF-ONE-WINNER" ]
  ]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryCacheRejoinJointFence"
    "ASPProof/SearchRouteAdmissionRetryCacheRejoinJointFence.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinJointFence
