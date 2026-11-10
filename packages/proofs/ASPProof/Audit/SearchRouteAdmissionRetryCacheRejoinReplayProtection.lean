-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Receipt
import ASPProof.SearchRouteAdmissionRetryCacheRejoinReplayProtection

namespace ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinReplayProtection

open ASPProof.Audit.Core
open ASPProof.SearchRouteAdmissionRetryCacheRejoinReplayProtection

def targets : List Target :=
  [ Target.mk
      ``supported_chain_claim_matches_root_scope
      "receipt-chain-epoch-bridge"
      [ "ASP-RFC-10.05-CRRP-EPOCH" ]
  , Target.mk
      ``permitted_new_attempt_advances_epoch
      "monotone-attempt-advance"
      [ "ASP-RFC-10.05-CRRP-EPOCH"
      , "ASP-RFC-10.05-CRRP-SUPERSESSION"
      ]
  , Target.mk
      ``non_increasing_epoch_cannot_begin
      "non-increasing-epoch-counterexample"
      [ "ASP-RFC-10.05-CRRP-EPOCH"
      , "ASP-RFC-10.05-CRRP-SUPERSESSION"
      ]
  , Target.mk
      ``activatable_claim_is_authority_current
      "authority-current-activation"
      [ "ASP-RFC-10.05-CRRP-CURRENT" ]
  , Target.mk
      ``cross_replica_claim_cannot_activate
      "cross-replica-counterexample"
      [ "ASP-RFC-10.05-CRRP-CURRENT" ]
  , Target.mk
      ``future_unissued_attempt_cannot_activate
      "future-epoch-counterexample"
      [ "ASP-RFC-10.05-CRRP-CURRENT"
      , "ASP-RFC-10.05-CRRP-NONIMPLICATION"
      ]
  , Target.mk
      ``newer_attempt_rejects_old_chain_activation
      "stale-activation-counterexample"
      [ "ASP-RFC-10.05-CRRP-SUPERSESSION"
      , "ASP-RFC-10.05-CRRP-NONIMPLICATION"
      ]
  , Target.mk
      ``activated_chain_cannot_activate_twice
      "duplicate-activation-counterexample"
      [ "ASP-RFC-10.05-CRRP-UNIQUE"
      , "ASP-RFC-10.05-CRRP-IDEMPOTENCE"
      ]
  , Target.mk
      ``activated_chain_is_idempotently_observable
      "idempotent-observation"
      [ "ASP-RFC-10.05-CRRP-IDEMPOTENCE" ]
  , Target.mk
      ``conflicting_terminal_digest_is_not_idempotent
      "terminal-conflict-counterexample"
      [ "ASP-RFC-10.05-CRRP-CONFLICT" ]
  , Target.mk
      ``active_chain_is_unique
      "single-active-chain"
      [ "ASP-RFC-10.05-CRRP-UNIQUE" ]
  , Target.mk
      ``newer_attempt_supersedes_old_observation
      "stale-observation-counterexample"
      [ "ASP-RFC-10.05-CRRP-SUPERSESSION" ]
  ]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryCacheRejoinReplayProtection"
    "ASPProof/SearchRouteAdmissionRetryCacheRejoinReplayProtection.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinReplayProtection
