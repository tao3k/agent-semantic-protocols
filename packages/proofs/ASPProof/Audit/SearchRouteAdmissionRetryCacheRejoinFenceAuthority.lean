-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Receipt
import ASPProof.SearchRouteAdmissionRetryCacheRejoinFenceAuthority

namespace ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinFenceAuthority

open ASPProof.Audit.Core
open ASPProof.SearchRouteAdmissionRetryCacheRejoinFenceAuthority

def targets : List Target :=
  [ Target.mk ``authority_commit_records_bound_durable_receipt
      "durable-fence-receipt" [ "ASP-RFC-10.05-CRFA-RECEIPT" ]
  , Target.mk ``stale_term_command_cannot_commit
      "stale-term-counterexample" [ "ASP-RFC-10.05-CRFA-TERM" ]
  , Target.mk ``nonleader_command_cannot_commit
      "nonleader-counterexample" [ "ASP-RFC-10.05-CRFA-LEADER" ]
  , Target.mk ``leader_change_fences_old_term
      "leader-change-fencing" [ "ASP-RFC-10.05-CRFA-LEADER" ]
  , Target.mk ``committed_receipt_is_queryable_after_lost_response
      "lost-response-recovery" [ "ASP-RFC-10.05-CRFA-RECOVERY" ]
  , Target.mk ``later_commit_preserves_existing_receipt
      "append-only-receipt-ledger" [ "ASP-RFC-10.05-CRFA-LEDGER" ]
  ]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryCacheRejoinFenceAuthority"
    "ASPProof/SearchRouteAdmissionRetryCacheRejoinFenceAuthority.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinFenceAuthority
