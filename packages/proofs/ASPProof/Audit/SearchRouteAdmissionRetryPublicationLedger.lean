-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteAdmissionRetryPublicationLedger

namespace ASPProof.Audit.SearchRouteAdmissionRetryPublicationLedger

open ASPProof.Audit.Core

def targets : List Target :=
  [ Target.mk
      ``ASPProof.SearchRouteAdmissionRetryPublicationLedger.existing_winner_is_returned_without_republication
      "lookup-first recovery"
      [ "ASP-RFC-10.05-RPL-LOOKUP-FIRST"
      , "ASP-RFC-10.05-RPL-RECOVERY" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryPublicationLedger.invalid_candidate_cannot_win_empty_key
      "invalid empty-cell candidate"
      ["ASP-RFC-10.05-RPL-EMPTY-INSTALL"],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryPublicationLedger.valid_candidate_wins_empty_key
      "valid empty-cell installation"
      ["ASP-RFC-10.05-RPL-EMPTY-INSTALL"],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryPublicationLedger.installed_winner_is_sticky_for_any_later_candidate
      "winner stickiness"
      ["ASP-RFC-10.05-RPL-WINNER-STICKY"],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryPublicationLedger.invalid_losing_retry_still_recovers_existing_winner
      "loser recovery"
      [ "ASP-RFC-10.05-RPL-LOOKUP-FIRST"
      , "ASP-RFC-10.05-RPL-RECOVERY"
      , "ASP-RFC-10.05-RPL-NONIMPLICATION" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryPublicationLedger.non_atomic_check_then_write_admits_two_distinct_winner_claims
      "double-winner counterexample"
      [ "ASP-RFC-10.05-RPL-EMPTY-INSTALL"
      , "ASP-RFC-10.05-RPL-NONIMPLICATION" ] ]

def auditJson :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryPublicationLedger"
    "ASPProof/SearchRouteAdmissionRetryPublicationLedger.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryPublicationLedger
