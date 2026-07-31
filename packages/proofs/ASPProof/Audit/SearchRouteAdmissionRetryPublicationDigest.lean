import ASPProof.Audit.Core
import ASPProof.SearchRouteAdmissionRetryPublicationDigest

namespace ASPProof.Audit.SearchRouteAdmissionRetryPublicationDigest

open ASPProof.Audit.Core

def targets : List Target :=
  [ Target.mk
      ``ASPProof.SearchRouteAdmissionRetryPublicationDigest.collision_free_digest_equality_preserves_domain_and_payload
      "conditional digest equality"
      [ "ASP-RFC-10.05-RPDB-COLLISION-ASSUMPTION"
      , "ASP-RFC-10.05-RPDB-EQUALITY" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryPublicationDigest.domain_tagged_preimages_do_not_make_arbitrary_digest_collision_free
      "constant-digest counterexample"
      [ "ASP-RFC-10.05-RPDB-COLLISION-ASSUMPTION"
      , "ASP-RFC-10.05-RPDB-NONIMPLICATION" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryPublicationDigest.omitting_domain_tag_aliases_equal_payloads_across_protocols
      "domain-tag counterexample"
      [ "ASP-RFC-10.05-RPDB-DOMAIN"
      , "ASP-RFC-10.05-RPDB-NONIMPLICATION" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryPublicationDigest.omitting_algorithm_identity_aliases_distinct_digest_references
      "algorithm-identity counterexample"
      [ "ASP-RFC-10.05-RPDB-ALGORITHM"
      , "ASP-RFC-10.05-RPDB-NONIMPLICATION" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryPublicationDigest.digest_truncation_is_not_collision_free
      "digest-truncation counterexample"
      [ "ASP-RFC-10.05-RPDB-NO-TRUNCATION"
      , "ASP-RFC-10.05-RPDB-NONIMPLICATION" ] ]

def auditJson :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryPublicationDigest"
    "ASPProof/SearchRouteAdmissionRetryPublicationDigest.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryPublicationDigest
