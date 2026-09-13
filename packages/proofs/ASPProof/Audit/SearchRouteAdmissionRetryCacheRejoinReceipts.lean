-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Receipt
import ASPProof.SearchRouteAdmissionRetryCacheRejoinReceipts

namespace ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinReceipts

open ASPProof.Audit.Core
open ASPProof.SearchRouteAdmissionRetryCacheRejoinReceipts

def targets : List Target :=
  [ Target.mk
      ``reads_enabled_record_has_complete_ordered_receipt_chain
      "phase-support-completeness"
      [ "ASP-RFC-10.05-CRTR-PHASE"
      , "ASP-RFC-10.05-CRTR-PREDECESSOR"
      , "ASP-RFC-10.05-CRTR-ORDER"
      ]
  , Target.mk
      ``complete_strict_receipt_chain_supports_reads_enabled_phase
      "constructive-receipt-support"
      [ "ASP-RFC-10.05-CRTR-RECEIPT"
      , "ASP-RFC-10.05-CRTR-ORDER"
      ]
  , Target.mk
      ``torn_token_receipt_cannot_support_token_issued_phase
      "torn-publication-counterexample"
      [ "ASP-RFC-10.05-CRTR-ATOMICITY"
      , "ASP-RFC-10.05-CRTR-NONIMPLICATION"
      ]
  , Target.mk
      ``later_read_receipt_cannot_replace_missing_predecessor
      "missing-predecessor-counterexample"
      [ "ASP-RFC-10.05-CRTR-PREDECESSOR"
      , "ASP-RFC-10.05-CRTR-NONIMPLICATION"
      ]
  , Target.mk
      ``present_but_reordered_receipts_cannot_support_membership_phase
      "reordered-receipt-counterexample"
      [ "ASP-RFC-10.05-CRTR-ORDER"
      , "ASP-RFC-10.05-CRTR-NONIMPLICATION"
      ]
  , Target.mk
      ``duplicate_receipt_generation_cannot_establish_strict_transition
      "duplicate-generation-counterexample"
      [ "ASP-RFC-10.05-CRTR-ORDER"
      , "ASP-RFC-10.05-CRTR-NONIMPLICATION"
      ]
  ]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryCacheRejoinReceipts"
    "ASPProof/SearchRouteAdmissionRetryCacheRejoinReceipts.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinReceipts
