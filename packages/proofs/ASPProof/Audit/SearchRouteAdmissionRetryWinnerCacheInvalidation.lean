-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteAdmissionRetryWinnerCacheInvalidation

namespace ASPProof.Audit.SearchRouteAdmissionRetryWinnerCacheInvalidation

open ASPProof.Audit.Core

def targets : List Target :=
  [ Target.mk
      ``ASPProof.SearchRouteAdmissionRetryWinnerCacheInvalidation.current_active_authorization_allows_cache_hit
      "current cache hit"
      ["ASP-RFC-10.05-RWCI-LOOKUP"],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryWinnerCacheInvalidation.same_generation_revocation_turns_prior_entry_into_miss
      "revocation invalidation"
      ["ASP-RFC-10.05-RWCI-REVOCATION"],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryWinnerCacheInvalidation.generation_advance_turns_reactivated_grant_entry_into_miss
      "generation invalidation"
      ["ASP-RFC-10.05-RWCI-GENERATION"],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryWinnerCacheInvalidation.purge_removes_entry_that_is_no_longer_authorized
      "logical purge"
      ["ASP-RFC-10.05-RWCI-PURGE"],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryWinnerCacheInvalidation.generation_free_cache_lookup_replays_stale_winner
      "stale-cache counterexample"
      ["ASP-RFC-10.05-RWCI-NONIMPLICATION"] ]

def auditJson :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryWinnerCacheInvalidation"
    "ASPProof/SearchRouteAdmissionRetryWinnerCacheInvalidation.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryWinnerCacheInvalidation
