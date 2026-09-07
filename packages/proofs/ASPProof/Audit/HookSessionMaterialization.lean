-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.HookSessionMaterialization

namespace ASPProof.Audit.HookSessionMaterialization

open Lean Elab Term
open ASPProof.Audit.Core

def targets : List Target :=
  [ { name := `ASPProof.HookSessionMaterialization.first_claim_is_admissible
      theoremFamily := "materialization-claim-admission"
      rfcClauseIds := ["ASP-RFC-10.05-EORM-CLAIM"] }
  , { name := `ASPProof.HookSessionMaterialization.committed_claim_rejects_competing_writer
      theoremFamily := "materialization-one-winner"
      rfcClauseIds := ["ASP-RFC-10.05-EORM-ONE-WINNER"] }
  , { name := `ASPProof.HookSessionMaterialization.claim_advances_generation_and_revision
      theoremFamily := "claim-generation-linearization"
      rfcClauseIds := ["ASP-RFC-10.05-EORM-CLAIM"] }
  , { name := `ASPProof.HookSessionMaterialization.duplicate_host_spawn_is_idempotent
      theoremFamily := "native-host-idempotency"
      rfcClauseIds := ["ASP-RFC-10.05-EORM-HOST-KEY"] }
  , { name := `ASPProof.HookSessionMaterialization.conflicting_host_claim_cannot_replace_winner
      theoremFamily := "native-host-conflict-rejection"
      rfcClauseIds := ["ASP-RFC-10.05-EORM-HOST-KEY",
        "ASP-RFC-10.05-EORM-ONE-WINNER"] }
  , { name := `ASPProof.HookSessionMaterialization.crash_after_host_spawn_reuses_same_child
      theoremFamily := "spawn-crash-recovery"
      rfcClauseIds := ["ASP-RFC-10.05-EORM-HOST-KEY",
        "ASP-RFC-10.05-EORM-SPAWN-RECEIPT"] }
  , { name := `ASPProof.HookSessionMaterialization.recorded_spawn_reconciles_instead_of_respawning
      theoremFamily := "spawned-state-reconciliation"
      rfcClauseIds := ["ASP-RFC-10.05-EORM-RECONCILE"] }
  , { name := `ASPProof.HookSessionMaterialization.binding_requires_exact_generation_and_child
      theoremFamily := "binding-identity-admission"
      rfcClauseIds := ["ASP-RFC-10.05-EORM-BIND"] }
  , { name := `ASPProof.HookSessionMaterialization.bound_state_requires_heartbeat
      theoremFamily := "heartbeat-before-activation"
      rfcClauseIds := ["ASP-RFC-10.05-EORM-LEASE"] }
  , { name := `ASPProof.HookSessionMaterialization.fresh_active_state_dispatches
      theoremFamily := "fresh-lease-dispatch"
      rfcClauseIds := ["ASP-RFC-10.05-EORM-DISPATCH"] }
  , { name := `ASPProof.HookSessionMaterialization.bound_dispatch_receipt_is_admissible
      theoremFamily := "dispatch-receipt-admission"
      rfcClauseIds := ["ASP-RFC-10.05-EORM-DISPATCH"] }
  , { name := `ASPProof.HookSessionMaterialization.expired_heartbeat_blocks_dispatch
      theoremFamily := "expired-lease-counterexample"
      rfcClauseIds := ["ASP-RFC-10.05-EORM-LEASE",
        "ASP-RFC-10.05-EORM-DISPATCH"] }
  , { name := `ASPProof.HookSessionMaterialization.expired_active_state_requires_replacement
      theoremFamily := "expired-lease-replacement"
      rfcClauseIds := ["ASP-RFC-10.05-EORM-DISPATCH"] }
  ]

def auditJson : TermElabM Json :=
  proofAuditJson
    "ASPProof.HookSessionMaterialization"
    "ASPProof/HookSessionMaterialization.lean"
    targets

end ASPProof.Audit.HookSessionMaterialization
