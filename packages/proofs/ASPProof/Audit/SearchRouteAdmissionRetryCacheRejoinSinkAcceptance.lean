-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Receipt
import ASPProof.SearchRouteAdmissionRetryCacheRejoinSinkAcceptance

namespace ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinSinkAcceptance

open ASPProof.Audit.Core
open ASPProof.SearchRouteAdmissionRetryCacheRejoinSinkAcceptance

def targets : List Target :=
  [ Target.mk
      ``publication_retry_preserves_delivery_identity
      "stable-delivery-identity"
      [ "ASP-RFC-10.05-CRSA-IDENTITY" ]
  , Target.mk
      ``admitted_receipt_matches_expected_delivery
      "receipt-delivery-binding"
      [ "ASP-RFC-10.05-CRSA-SINK"
      , "ASP-RFC-10.05-CRSA-VERSION"
      , "ASP-RFC-10.05-CRSA-EDGE"
      ]
  , Target.mk
      ``verified_receipt_closes_publication_obligation
      "verified-receipt-closure"
      [ "ASP-RFC-10.05-CRSA-ACK" ]
  , Target.mk
      ``cross_sink_receipt_cannot_acknowledge
      "cross-sink-counterexample"
      [ "ASP-RFC-10.05-CRSA-SINK" ]
  , Target.mk
      ``wrong_protocol_version_receipt_cannot_acknowledge
      "wrong-version-counterexample"
      [ "ASP-RFC-10.05-CRSA-VERSION" ]
  , Target.mk
      ``wrong_edge_receipt_cannot_acknowledge
      "wrong-edge-counterexample"
      [ "ASP-RFC-10.05-CRSA-EDGE" ]
  , Target.mk
      ``unverifiable_receipt_cannot_acknowledge
      "unverifiable-receipt-counterexample"
      [ "ASP-RFC-10.05-CRSA-VERIFY" ]
  , Target.mk
      ``stale_edge_receipt_cannot_acknowledge
      "stale-receipt-counterexample"
      [ "ASP-RFC-10.05-CRSA-STALE" ]
  , Target.mk
      ``receipt_closed_bundle_cannot_acknowledge_again
      "post-receipt-closure-counterexample"
      [ "ASP-RFC-10.05-CRSA-ACK" ]
  ]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryCacheRejoinSinkAcceptance"
    "ASPProof/SearchRouteAdmissionRetryCacheRejoinSinkAcceptance.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinSinkAcceptance
