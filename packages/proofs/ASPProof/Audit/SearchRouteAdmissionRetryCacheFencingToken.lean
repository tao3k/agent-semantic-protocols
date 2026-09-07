-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteAdmissionRetryCacheFencingToken

namespace ASPProof.Audit.SearchRouteAdmissionRetryCacheFencingToken

open ASPProof.Audit.Core

def targets : List Target :=
  [ Target.mk
      ``ASPProof.SearchRouteAdmissionRetryCacheFencingToken.serving_token_is_authentic_and_generation_current
      "serving-token validity"
      [ "ASP-RFC-10.05-CSFT-AUTHENTICITY"
      , "ASP-RFC-10.05-CSFT-CURRENT" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryCacheFencingToken.monotonic_fence_advance_keeps_old_token_stale
      "monotonic fencing"
      ["ASP-RFC-10.05-CSFT-MONOTONE"],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryCacheFencingToken.formerly_authentic_old_token_cannot_serve_after_fence_advance
      "old-token counterexample"
      ["ASP-RFC-10.05-CSFT-NONIMPLICATION"],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryCacheFencingToken.forged_current_generation_without_issuance_cannot_serve
      "forged-current counterexample"
      [ "ASP-RFC-10.05-CSFT-AUTHENTICITY"
      , "ASP-RFC-10.05-CSFT-NONIMPLICATION" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryCacheFencingToken.boolean_serving_flag_can_reenable_replica_with_stale_token
      "boolean-serving counterexample"
      [ "ASP-RFC-10.05-CSFT-TOKEN"
      , "ASP-RFC-10.05-CSFT-REJOIN"
      , "ASP-RFC-10.05-CSFT-NONIMPLICATION" ] ]

def auditJson :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryCacheFencingToken"
    "ASPProof/SearchRouteAdmissionRetryCacheFencingToken.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryCacheFencingToken
