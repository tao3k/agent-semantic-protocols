-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteAdmissionRetryCacheRejoinDeterministicRecoveryContention

namespace ASPProof.Audit

def writeReceipt
    (path : System.FilePath)
    (auditJson : Lean.Elab.TermElabM Lean.Json) :
    Lean.Elab.Command.CommandElabM Unit := do
  let audit ← Lean.Elab.Command.liftTermElabM auditJson
  IO.FS.writeFile path (audit.pretty ++ "\n")
  Lean.logInfo m!"wrote {path}"

end ASPProof.Audit

namespace ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinDeterministicRecoveryContention

open ASPProof.Audit.Core
open ASPProof.SearchRouteAdmissionRetryCacheRejoinDeterministicRecoveryContention

def targets : List Target := [
  Target.mk
    ``two_candidate_winner_exists
    "bounded-contention-winner-existence"
    ["DRC-EXISTENCE", "DRC-WINNER"],
  Target.mk
    ``injective_rank_makes_winner_unique
    "injective-rank-single-winner"
    ["DRC-RANK", "DRC-UNIQUENESS"],
  Target.mk
    ``noninjective_rank_can_admit_two_distinct_winners
    "rank-collision-counterexample"
    ["DRC-COUNTEREXAMPLE", "DRC-RANK"],
  Target.mk
    ``winner_commit_advances_generation
    "winner-generation-advance"
    ["DRC-COMMIT", "DRC-EPOCH"],
  Target.mk
    ``previous_epoch_is_fenced_after_winner_commit
    "previous-epoch-fence"
    ["DRC-FENCE", "DRC-EPOCH"],
  Target.mk
    ``loser_redirect_names_a_distinct_winner
    "loser-winner-separation"
    ["DRC-LOSER", "DRC-REDIRECT"],
  Target.mk
    ``contention_round_compatibility_binds_candidate_set
    "round-candidate-set-binding"
    ["DRC-ROUND", "DRC-IDENTITY"],
  Target.mk
    ``changed_candidate_set_invalidates_contention_round
    "candidate-set-cache-invalidation"
    ["DRC-CACHE", "DRC-CANDIDATES"],
  Target.mk
    ``same_winner_does_not_imply_round_compatibility
    "same-winner-round-separation"
    ["DRC-COUNTEREXAMPLE", "DRC-ROUND"],
  Target.mk
    ``stale_epoch_blocks_contention_commit
    "stale-epoch-commit-rejection"
    ["DRC-COMMIT", "DRC-EPOCH"],
  Target.mk
    ``stale_revision_blocks_contention_commit
    "stale-revision-commit-rejection"
    ["DRC-COMMIT", "DRC-REVISION"]
]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryCacheRejoinDeterministicRecoveryContention"
    "ASPProof/SearchRouteAdmissionRetryCacheRejoinDeterministicRecoveryContention.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinDeterministicRecoveryContention
