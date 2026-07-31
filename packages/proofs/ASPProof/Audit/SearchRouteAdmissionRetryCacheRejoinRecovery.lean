import ASPProof.Audit.Core
import ASPProof.SearchRouteAdmissionRetryCacheRejoinRecovery

namespace ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinRecovery

open ASPProof.Audit.Core

def targets : List Target :=
  [ Target.mk
      ``ASPProof.SearchRouteAdmissionRetryCacheRejoinRecovery.resume_rejoin_never_decreases_phase
      "monotonic recovery"
      ["ASP-RFC-10.05-CRCR-MONOTONE"],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryCacheRejoinRecovery.resume_preserves_well_formed_rejoin_state
      "recovery invariant"
      [ "ASP-RFC-10.05-CRCR-TOKEN"
      , "ASP-RFC-10.05-CRCR-IDEMPOTENT" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryCacheRejoinRecovery.complete_recovery_issues_one_token_and_enables_reads_last
      "complete recovery"
      [ "ASP-RFC-10.05-CRCR-TOKEN"
      , "ASP-RFC-10.05-CRCR-READ" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryCacheRejoinRecovery.enabling_reads_from_partial_rejoin_is_not_well_formed
      "premature-read counterexample"
      [ "ASP-RFC-10.05-CRCR-READ"
      , "ASP-RFC-10.05-CRCR-NONIMPLICATION" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryCacheRejoinRecovery.reissuing_token_after_token_phase_breaks_exactly_once_recovery
      "duplicate-token counterexample"
      [ "ASP-RFC-10.05-CRCR-TOKEN"
      , "ASP-RFC-10.05-CRCR-NONIMPLICATION" ] ]

def auditJson :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryCacheRejoinRecovery"
    "ASPProof/SearchRouteAdmissionRetryCacheRejoinRecovery.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinRecovery
