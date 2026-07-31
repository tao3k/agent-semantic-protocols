import ASPProof.Audit.Core
import ASPProof.SearchRouteAdmissionRetryPublicationKey

namespace ASPProof.Audit.SearchRouteAdmissionRetryPublicationKey

open ASPProof.Audit.Core

def targets : List Target :=
  [ Target.mk
      ``ASPProof.SearchRouteAdmissionRetryPublicationKey.full_key_equality_preserves_all_scope_dimensions
      "full-key identity"
      ["ASP-RFC-10.05-RPKS-FULL-KEY"],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryPublicationKey.omitting_workspace_aliases_distinct_publication_domains
      "workspace omission counterexample"
      [ "ASP-RFC-10.05-RPKS-WORKSPACE"
      , "ASP-RFC-10.05-RPKS-NONIMPLICATION" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryPublicationKey.omitting_generation_aliases_distinct_publication_domains
      "generation omission counterexample"
      [ "ASP-RFC-10.05-RPKS-GENERATION"
      , "ASP-RFC-10.05-RPKS-NONIMPLICATION" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryPublicationKey.omitting_contract_version_aliases_distinct_publication_domains
      "contract omission counterexample"
      [ "ASP-RFC-10.05-RPKS-CONTRACT"
      , "ASP-RFC-10.05-RPKS-NONIMPLICATION" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryPublicationKey.omitting_tenant_scope_aliases_distinct_publication_domains
      "tenant omission counterexample"
      [ "ASP-RFC-10.05-RPKS-TENANT"
      , "ASP-RFC-10.05-RPKS-NONIMPLICATION" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryPublicationKey.omitting_retry_identity_aliases_distinct_publication_domains
      "retry omission counterexample"
      [ "ASP-RFC-10.05-RPKS-RETRY"
      , "ASP-RFC-10.05-RPKS-NONIMPLICATION" ] ]

def auditJson :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryPublicationKey"
    "ASPProof/SearchRouteAdmissionRetryPublicationKey.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryPublicationKey
