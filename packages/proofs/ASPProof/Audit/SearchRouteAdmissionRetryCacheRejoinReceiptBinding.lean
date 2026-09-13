-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Receipt
import ASPProof.SearchRouteAdmissionRetryCacheRejoinReceiptBinding

namespace ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinReceiptBinding

open ASPProof.Audit.Core
open ASPProof.SearchRouteAdmissionRetryCacheRejoinReceiptBinding

def targets : List Target :=
  [ Target.mk
      ``supported_reads_chain_has_rooted_state_receipt
      "rooted-state-receipt"
      [ "ASP-RFC-10.05-CRCB-ROOT" ]
  , Target.mk
      ``supported_reads_chain_has_single_attempt_and_replica
      "single-scope-chain"
      [ "ASP-RFC-10.05-CRCB-SCOPE"
      , "ASP-RFC-10.05-CRCB-NONIMPLICATION"
      ]
  , Target.mk
      ``supported_reads_chain_is_predecessor_digest_linked
      "predecessor-digest-chain"
      [ "ASP-RFC-10.05-CRCB-LINK" ]
  , Target.mk
      ``cross_attempt_receipts_cannot_support_reads
      "cross-attempt-counterexample"
      [ "ASP-RFC-10.05-CRCB-SCOPE"
      , "ASP-RFC-10.05-CRCB-NONIMPLICATION"
      ]
  , Target.mk
      ``cross_replica_receipts_cannot_support_reads
      "cross-replica-counterexample"
      [ "ASP-RFC-10.05-CRCB-SCOPE"
      , "ASP-RFC-10.05-CRCB-NONIMPLICATION"
      ]
  , Target.mk
      ``detached_predecessor_digest_cannot_support_reads
      "detached-predecessor-counterexample"
      [ "ASP-RFC-10.05-CRCB-LINK"
      , "ASP-RFC-10.05-CRCB-NONIMPLICATION"
      ]
  , Target.mk
      ``equal_receipt_digest_implies_equal_content
      "injective-content-commitment"
      [ "ASP-RFC-10.05-CRCB-CONTENT"
      , "ASP-RFC-10.05-CRCB-DIGEST"
      ]
  , Target.mk
      ``changed_result_cannot_reuse_receipt_digest
      "result-substitution-counterexample"
      [ "ASP-RFC-10.05-CRCB-CONTENT"
      , "ASP-RFC-10.05-CRCB-DIGEST"
      ]
  ]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryCacheRejoinReceiptBinding"
    "ASPProof/SearchRouteAdmissionRetryCacheRejoinReceiptBinding.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinReceiptBinding
