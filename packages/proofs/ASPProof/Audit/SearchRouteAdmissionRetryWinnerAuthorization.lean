import ASPProof.Audit.Core
import ASPProof.SearchRouteAdmissionRetryWinnerAuthorization

namespace ASPProof.Audit.SearchRouteAdmissionRetryWinnerAuthorization

open ASPProof.Audit.Core

def targets : List Target :=
  [ Target.mk
      ``ASPProof.SearchRouteAdmissionRetryWinnerAuthorization.authorization_preserves_key_and_all_scope_dimensions
      "authorization scope preservation"
      ["ASP-RFC-10.05-RWA-SCOPE"],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryWinnerAuthorization.unauthorized_request_is_denied_before_winner_lookup
      "authorization-before-lookup"
      [ "ASP-RFC-10.05-RWA-ACTIVE-GRANT"
      , "ASP-RFC-10.05-RWA-AUTH-BEFORE-LOOKUP" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryWinnerAuthorization.authorized_request_recovers_existing_winner
      "authorized winner release"
      ["ASP-RFC-10.05-RWA-RELEASE"],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryWinnerAuthorization.known_key_and_matching_scope_without_active_grant_is_not_authorized
      "inactive-grant counterexample"
      [ "ASP-RFC-10.05-RWA-ACTIVE-GRANT"
      , "ASP-RFC-10.05-RWA-NONIMPLICATION" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryWinnerAuthorization.active_grant_with_wrong_tenant_scope_is_not_authorized
      "tenant-scope counterexample"
      [ "ASP-RFC-10.05-RWA-SCOPE"
      , "ASP-RFC-10.05-RWA-NONIMPLICATION" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryWinnerAuthorization.lookup_before_authorization_leaks_winner_to_wrong_tenant
      "lookup-first leakage counterexample"
      [ "ASP-RFC-10.05-RWA-AUTH-BEFORE-LOOKUP"
      , "ASP-RFC-10.05-RWA-NONIMPLICATION" ] ]

def auditJson :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryWinnerAuthorization"
    "ASPProof/SearchRouteAdmissionRetryWinnerAuthorization.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryWinnerAuthorization
