-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.ActivationRepairLedger

namespace ASPProof.Audit.ActivationRepairLedger

open Lean Elab Term
open ASPProof.Audit.Core

def targets : List Target :=
  [ { name := `ASPProof.ActivationRepairLedger.stale_generation_cannot_replay
      theoremFamily := "stale-replay-rejection"
      rfcClauseIds := ["ASP-RFC-10.05-ARML-REPLAY"] }
  , { name := `ASPProof.ActivationRepairLedger.stale_generation_cannot_ack
      theoremFamily := "stale-ack-rejection"
      rfcClauseIds := ["ASP-RFC-10.05-ARML-ACK"] }
  , { name := `ASPProof.ActivationRepairLedger.current_ack_is_admissible
      theoremFamily := "current-ack-admission"
      rfcClauseIds := ["ASP-RFC-10.05-ARML-KEY",
        "ASP-RFC-10.05-ARML-ACK"] }
  , { name := `ASPProof.ActivationRepairLedger.old_ack_is_rejected
      theoremFamily := "old-ack-counterexample"
      rfcClauseIds := ["ASP-RFC-10.05-ARML-ACK"] }
  , { name := `ASPProof.ActivationRepairLedger.acknowledgement_reordering_preserves_head
      theoremFamily := "ack-reordering-head-preservation"
      rfcClauseIds := ["ASP-RFC-10.05-ARML-ACK"] }
  , { name := `ASPProof.ActivationRepairLedger.old_tombstone_is_collectable
      theoremFamily := "covered-tombstone-collection"
      rfcClauseIds := ["ASP-RFC-10.05-ARML-TOMBSTONE",
        "ASP-RFC-10.05-ARML-GC"] }
  , { name := `ASPProof.ActivationRepairLedger.compaction_preserves_head_and_gc_floor
      theoremFamily := "compaction-floor-preservation"
      rfcClauseIds := ["ASP-RFC-10.05-ARML-GC"] }
  , { name := `ASPProof.ActivationRepairLedger.gc_floor_blocks_old_generation_resurrection
      theoremFamily := "gc-resurrection-rejection"
      rfcClauseIds := ["ASP-RFC-10.05-ARML-IMPORT"] }
  , { name := `ASPProof.ActivationRepairLedger.deleting_tombstone_without_advancing_floor_allows_resurrection
      theoremFamily := "missing-floor-resurrection-counterexample"
      rfcClauseIds := ["ASP-RFC-10.05-ARML-TOMBSTONE",
        "ASP-RFC-10.05-ARML-IMPORT"] }
  , { name := `ASPProof.ActivationRepairLedger.current_entry_remains_replayable
      theoremFamily := "current-frontier-replay"
      rfcClauseIds := ["ASP-RFC-10.05-ARML-REPLAY",
        "ASP-RFC-10.05-ARML-FRONTIER"] }
  , { name := `ASPProof.ActivationRepairLedger.unresolved_frontier_dominates_full_ledger_recovery
      theoremFamily := "frontier-componentwise-cost"
      rfcClauseIds := ["ASP-RFC-10.05-ARML-COST"] }
  , { name := `ASPProof.ActivationRepairLedger.unresolved_frontier_strictly_reduces_rounds
      theoremFamily := "frontier-round-reduction"
      rfcClauseIds := ["ASP-RFC-10.05-ARML-COST"] }
  , { name := `ASPProof.ActivationRepairLedger.unresolved_frontier_strictly_reduces_tokens
      theoremFamily := "frontier-token-reduction"
      rfcClauseIds := ["ASP-RFC-10.05-ARML-COST"] }
  , { name := `ASPProof.ActivationRepairLedger.unresolved_frontier_satisfies_linear_token_bound
      theoremFamily := "linear-token-bound"
      rfcClauseIds := ["ASP-RFC-10.05-ARML-TOKEN-BOUND"] }
  ]

def auditJson : TermElabM Json :=
  proofAuditJson
    "ASPProof.ActivationRepairLedger"
    "ASPProof/ActivationRepairLedger.lean"
    targets

end ASPProof.Audit.ActivationRepairLedger
