import ASPProof.Audit.Receipt
import ASPProof.SearchRouteAdmissionRetryCacheRejoinPolicySnapshot

namespace ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinPolicySnapshot

open ASPProof.Audit.Core
open ASPProof.SearchRouteAdmissionRetryCacheRejoinPolicySnapshot

def targets : List Target :=
  [ Target.mk
      ``committable_acknowledgement_is_bound_to_current_snapshots
      "dual-snapshot-commit-gate"
      [ "ASP-RFC-10.05-CRPS-OBSERVATION"
      , "ASP-RFC-10.05-CRPS-GATE"
      ]
  , Target.mk
      ``committed_acknowledgement_records_snapshots_and_closes
      "snapshot-bound-closure"
      [ "ASP-RFC-10.05-CRPS-COMMIT" ]
  , Target.mk
      ``stale_admission_snapshot_cannot_commit_acknowledgement
      "stale-admission-counterexample"
      [ "ASP-RFC-10.05-CRPS-STALE" ]
  , Target.mk
      ``stale_policy_revision_cannot_commit_acknowledgement
      "stale-policy-counterexample"
      [ "ASP-RFC-10.05-CRPS-STALE" ]
  , Target.mk
      ``wrong_policy_digest_cannot_commit_acknowledgement
      "wrong-policy-digest-counterexample"
      [ "ASP-RFC-10.05-CRPS-POLICY"
      , "ASP-RFC-10.05-CRPS-STALE"
      ]
  , Target.mk
      ``newer_policy_snapshot_rejects_old_validation
      "policy-race-counterexample"
      [ "ASP-RFC-10.05-CRPS-STALE" ]
  , Target.mk
      ``changed_admission_snapshot_rejects_old_validation
      "admission-race-counterexample"
      [ "ASP-RFC-10.05-CRPS-STALE" ]
  , Target.mk
      ``equal_bound_policy_digest_implies_equal_lifecycle
      "injective-policy-commitment"
      [ "ASP-RFC-10.05-CRPS-DIGEST" ]
  ]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryCacheRejoinPolicySnapshot"
    "ASPProof/SearchRouteAdmissionRetryCacheRejoinPolicySnapshot.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinPolicySnapshot
