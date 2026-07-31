import ASPProof.Audit.Core
import ASPProof.SearchRouteAdmissionRetryAuthorizationStamp

namespace ASPProof.Audit.SearchRouteAdmissionRetryAuthorizationStamp

open ASPProof.Audit.Core

def targets : List Target :=
  [ Target.mk
      ``ASPProof.SearchRouteAdmissionRetryAuthorizationStamp.safe_release_preserves_decision_generation_and_current_activity
      "release-stamp safety"
      [ "ASP-RFC-10.05-RAS-GENERATION"
      , "ASP-RFC-10.05-RAS-ACTIVITY"
      , "ASP-RFC-10.05-RAS-RELEASE" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryAuthorizationStamp.failed_freshness_recheck_denies_release
      "fail-closed freshness recheck"
      ["ASP-RFC-10.05-RAS-RELEASE"],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryAuthorizationStamp.original_active_generation_allows_release
      "current-generation release"
      ["ASP-RFC-10.05-RAS-RELEASE"],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryAuthorizationStamp.same_generation_revocation_invalidates_prior_decision
      "revocation counterexample"
      [ "ASP-RFC-10.05-RAS-ACTIVITY"
      , "ASP-RFC-10.05-RAS-NONIMPLICATION" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryAuthorizationStamp.generation_advance_invalidates_reactivated_grant_decision
      "generation-replay counterexample"
      [ "ASP-RFC-10.05-RAS-GENERATION"
      , "ASP-RFC-10.05-RAS-REPLAY"
      , "ASP-RFC-10.05-RAS-NONIMPLICATION" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryAuthorizationStamp.decision_bit_without_freshness_recheck_releases_stale_winner
      "TOCTOU counterexample"
      [ "ASP-RFC-10.05-RAS-ATOMICITY"
      , "ASP-RFC-10.05-RAS-NONIMPLICATION" ] ]

def auditJson :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryAuthorizationStamp"
    "ASPProof/SearchRouteAdmissionRetryAuthorizationStamp.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryAuthorizationStamp
