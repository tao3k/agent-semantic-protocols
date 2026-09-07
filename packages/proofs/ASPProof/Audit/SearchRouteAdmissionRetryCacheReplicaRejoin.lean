-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteAdmissionRetryCacheReplicaRejoin

namespace ASPProof.Audit.SearchRouteAdmissionRetryCacheReplicaRejoin

open ASPProof.Audit.Core

def targets : List Target :=
  [ Target.mk
      ``ASPProof.SearchRouteAdmissionRetryCacheReplicaRejoin.safe_rejoin_read_carries_state_token_membership_and_order
      "safe-rejoin certificate"
      ["ASP-RFC-10.05-CRRO-ORDER"],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryCacheReplicaRejoin.caught_up_replica_with_ordered_receipt_may_read
      "ordered rejoin"
      ["ASP-RFC-10.05-CRRO-READ"],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryCacheReplicaRejoin.current_token_and_membership_before_state_catchup_are_unsafe
      "pre-catchup counterexample"
      [ "ASP-RFC-10.05-CRRO-STATE"
      , "ASP-RFC-10.05-CRRO-NONIMPLICATION" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryCacheReplicaRejoin.final_current_state_without_order_receipt_does_not_prove_safe_rejoin
      "final-state counterexample"
      [ "ASP-RFC-10.05-CRRO-ORDER"
      , "ASP-RFC-10.05-CRRO-NONIMPLICATION" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryCacheReplicaRejoin.state_catchup_and_order_do_not_authorize_forged_token
      "forged-token counterexample"
      [ "ASP-RFC-10.05-CRRO-TOKEN"
      , "ASP-RFC-10.05-CRRO-NONIMPLICATION" ] ]

def auditJson :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryCacheReplicaRejoin"
    "ASPProof/SearchRouteAdmissionRetryCacheReplicaRejoin.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryCacheReplicaRejoin
