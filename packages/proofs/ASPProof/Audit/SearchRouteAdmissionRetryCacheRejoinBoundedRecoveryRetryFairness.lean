-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteAdmissionRetryCacheRejoinBoundedRecoveryRetryFairness

namespace ASPProof.Audit

def writeReceipt
    (path : System.FilePath)
    (auditJson : Lean.Elab.TermElabM Lean.Json) :
    Lean.Elab.Command.CommandElabM Unit := do
  let audit ← Lean.Elab.Command.liftTermElabM auditJson
  IO.FS.writeFile path (audit.pretty ++ "\n")
  Lean.logInfo m!"wrote {path}"

end ASPProof.Audit

namespace ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinBoundedRecoveryRetryFairness

open ASPProof.Audit.Core
open ASPProof.SearchRouteAdmissionRetryCacheRejoinBoundedRecoveryRetryFairness

def targets : List Target := [
  Target.mk
    ``next_retry_increments_round_and_attempt
    "retry-round-attempt-advance"
    ["BRRF-ROUND", "BRRF-ATTEMPT"],
  Target.mk
    ``authorized_retry_stays_within_attempt_budget
    "authorized-retry-budget-bound"
    ["BRRF-BUDGET", "BRRF-SAFETY"],
  Target.mk
    ``exhausted_attempt_budget_blocks_retry
    "exhausted-budget-rejection"
    ["BRRF-BUDGET", "BRRF-TERMINAL"],
  Target.mk
    ``previous_round_is_fenced_after_retry
    "previous-round-fence"
    ["BRRF-ROUND", "BRRF-FENCE"],
  Target.mk
    ``stale_round_blocks_retry_fence
    "stale-round-rejection"
    ["BRRF-ROUND", "BRRF-FENCE"],
  Target.mk
    ``total_backoff_is_bounded
    "cumulative-backoff-bound"
    ["BRRF-BACKOFF", "BRRF-BOUND"],
  Target.mk
    ``adversarial_scheduler_can_starve_a_candidate
    "unconditional-liveness-counterexample"
    ["BRRF-COUNTEREXAMPLE", "BRRF-FAIRNESS"],
  Target.mk
    ``conditional_bounded_recovery_liveness
    "fairness-conditional-liveness"
    ["BRRF-FAIRNESS", "BRRF-LIVENESS"],
  Target.mk
    ``retry_command_compatibility_binds_round
    "retry-command-round-binding"
    ["BRRF-COMMAND", "BRRF-IDENTITY"],
  Target.mk
    ``changed_round_rejects_retry_replay
    "cross-round-replay-rejection"
    ["BRRF-REPLAY", "BRRF-ROUND"],
  Target.mk
    ``same_candidate_does_not_imply_retry_compatibility
    "same-candidate-round-separation"
    ["BRRF-COUNTEREXAMPLE", "BRRF-IDENTITY"],
  Target.mk
    ``changed_attempt_budget_rejects_retry_replay
    "attempt-budget-cache-invalidation"
    ["BRRF-BUDGET", "BRRF-CACHE"]
]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryCacheRejoinBoundedRecoveryRetryFairness"
    "ASPProof/SearchRouteAdmissionRetryCacheRejoinBoundedRecoveryRetryFairness.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinBoundedRecoveryRetryFairness
