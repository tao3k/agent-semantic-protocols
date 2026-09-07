-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteAdmissionLedgerOrder

namespace ASPProof.Audit.SearchRouteAdmissionLedgerOrder

open ASPProof.Audit.Core

def targets : List Target :=
  [ Target.mk
      ``ASPProof.SearchRouteAdmissionLedgerOrder.happens_before_trans
      "ledger-order law"
      ["ASP-RFC-10.05-ALO-LEDGER-ORDER"],
    Target.mk
      ``ASPProof.SearchRouteAdmissionLedgerOrder.happens_before_irreflexive
      "ledger-order law"
      ["ASP-RFC-10.05-ALO-LEDGER-ORDER"],
    Target.mk
      ``ASPProof.SearchRouteAdmissionLedgerOrder.sequenced_certificate_tokens_are_authoritatively_issued
      "token authenticity"
      ["ASP-RFC-10.05-ALO-AUTHENTICITY"],
    Target.mk
      ``ASPProof.SearchRouteAdmissionLedgerOrder.sequenced_certificate_orders_both_tokens_before_publication
      "sequenced publication"
      ["ASP-RFC-10.05-ALO-PUBLICATION"],
    Target.mk
      ``ASPProof.SearchRouteAdmissionLedgerOrder.wall_clock_order_does_not_imply_ledger_order
      "ordering counterexample"
      [ "ASP-RFC-10.05-ALO-LEDGER-ORDER"
      , "ASP-RFC-10.05-ALO-NONIMPLICATION" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionLedgerOrder.previous_generation_sequence_does_not_imply_happens_before
      "generation counterexample"
      [ "ASP-RFC-10.05-ALO-GENERATION"
      , "ASP-RFC-10.05-ALO-NONIMPLICATION" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionLedgerOrder.permissive_verifier_does_not_establish_authenticity
      "authenticity counterexample"
      [ "ASP-RFC-10.05-ALO-AUTHENTICITY"
      , "ASP-RFC-10.05-ALO-NONIMPLICATION" ] ]

def auditJson :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionLedgerOrder"
    "ASPProof/SearchRouteAdmissionLedgerOrder.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionLedgerOrder
