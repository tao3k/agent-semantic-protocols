-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteAdmissionRetryPublicationKeyEncoding

namespace ASPProof.Audit.SearchRouteAdmissionRetryPublicationKeyEncoding

open ASPProof.Audit.Core

def targets : List Target :=
  [ Target.mk
      ``ASPProof.SearchRouteAdmissionRetryPublicationKeyEncoding.canonical_decode_encode_round_trip
      "canonical round trip"
      ["ASP-RFC-10.05-RPKE-ROUNDTRIP"],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryPublicationKeyEncoding.canonical_encoding_is_injective
      "canonical injectivity"
      ["ASP-RFC-10.05-RPKE-INJECTIVE"],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryPublicationKeyEncoding.delimiter_free_concatenation_is_not_injective
      "framing counterexample"
      [ "ASP-RFC-10.05-RPKE-FRAMING"
      , "ASP-RFC-10.05-RPKE-NONIMPLICATION" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryPublicationKeyEncoding.unordered_field_encoding_is_not_injective
      "field-order counterexample"
      [ "ASP-RFC-10.05-RPKE-ORDER"
      , "ASP-RFC-10.05-RPKE-NONIMPLICATION" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryPublicationKeyEncoding.omitting_encoding_schema_aliases_distinct_decoding_domains
      "schema-identity counterexample"
      [ "ASP-RFC-10.05-RPKE-SCHEMA"
      , "ASP-RFC-10.05-RPKE-NONIMPLICATION" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryPublicationKeyEncoding.lossy_normalization_is_not_identity_preserving
      "normalization counterexample"
      [ "ASP-RFC-10.05-RPKE-NORMALIZATION"
      , "ASP-RFC-10.05-RPKE-NONIMPLICATION" ] ]

def auditJson :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryPublicationKeyEncoding"
    "ASPProof/SearchRouteAdmissionRetryPublicationKeyEncoding.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryPublicationKeyEncoding
