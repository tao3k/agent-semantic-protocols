import ASPProof.Audit.Core
import ASPProof.SearchRouteAdmissionRetryWinnerNoninterference

namespace ASPProof.Audit.SearchRouteAdmissionRetryWinnerNoninterference

open ASPProof.Audit.Core

def targets : List Target :=
  [ Target.mk
      ``ASPProof.SearchRouteAdmissionRetryWinnerNoninterference.constant_denial_is_independent_of_winner_occupancy
      "unauthorized observation equality"
      ["ASP-RFC-10.05-RWNI-EQUALITY"],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryWinnerNoninterference.winner_existence_leaks_through_outcome_class
      "outcome leakage counterexample"
      [ "ASP-RFC-10.05-RWNI-OUTCOME"
      , "ASP-RFC-10.05-RWNI-NONIMPLICATION" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryWinnerNoninterference.winner_existence_leaks_through_denied_payload
      "payload leakage counterexample"
      [ "ASP-RFC-10.05-RWNI-PAYLOAD"
      , "ASP-RFC-10.05-RWNI-NONIMPLICATION" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryWinnerNoninterference.winner_existence_leaks_through_cache_mutation
      "cache leakage counterexample"
      [ "ASP-RFC-10.05-RWNI-CACHE"
      , "ASP-RFC-10.05-RWNI-NONIMPLICATION" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryWinnerNoninterference.winner_existence_leaks_through_protocol_timing_class
      "timing-class leakage counterexample"
      [ "ASP-RFC-10.05-RWNI-TIMING-CLASS"
      , "ASP-RFC-10.05-RWNI-NONIMPLICATION" ] ]

def auditJson :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryWinnerNoninterference"
    "ASPProof/SearchRouteAdmissionRetryWinnerNoninterference.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryWinnerNoninterference
