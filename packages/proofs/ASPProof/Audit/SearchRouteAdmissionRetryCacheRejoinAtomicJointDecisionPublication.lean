-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteAdmissionRetryCacheRejoinAtomicJointDecisionPublication

namespace ASPProof.Audit

def writeReceipt
    (path : System.FilePath)
    (auditJson : Lean.Elab.TermElabM Lean.Json) :
    Lean.Elab.Command.CommandElabM Unit := do
  let audit ← Lean.Elab.Command.liftTermElabM auditJson
  IO.FS.writeFile path (audit.pretty ++ "\n")
  Lean.logInfo m!"wrote {path}"

end ASPProof.Audit

namespace ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinAtomicJointDecisionPublication

open ASPProof.Audit.Core
open ASPProof.SearchRouteAdmissionRetryCacheRejoinAtomicJointDecisionPublication

def targets : List Target := [
  Target.mk
    ``publication_step_preserves_safety
    "publication-step-safety"
    ["AJDP-STATE", "AJDP-SAFETY"],
  Target.mk
    ``every_reachable_publication_state_is_safe
    "reachable-state-safety"
    ["AJDP-REACHABLE", "AJDP-SAFETY"],
  Target.mk
    ``reachable_new_only_implies_durable_decision
    "new-only-durable-decision"
    ["AJDP-PHASE", "AJDP-DURABLE"],
  Target.mk
    ``intent_cannot_advance_phase
    "intent-phase-advance-rejection"
    ["AJDP-INTENT", "AJDP-PHASE"],
  Target.mk
    ``durable_joint_state_can_advance
    "durable-phase-advance"
    ["AJDP-DURABLE", "AJDP-PHASE"],
  Target.mk
    ``intent_can_abort_without_phase_advance
    "invalid-intent-safe-abort"
    ["AJDP-INTENT", "AJDP-RECOVERY"],
  Target.mk
    ``publication_step_revision_is_monotone
    "publication-revision-monotonicity"
    ["AJDP-REVISION", "AJDP-MONOTONE"],
  Target.mk
    ``stale_revision_blocks_phase_cas
    "stale-revision-cas-rejection"
    ["AJDP-CAS", "AJDP-REVISION"],
  Target.mk
    ``non_durable_state_blocks_phase_cas
    "non-durable-cas-rejection"
    ["AJDP-CAS", "AJDP-DURABLE"],
  Target.mk
    ``changed_verification_snapshot_blocks_phase_advance
    "verification-snapshot-fence"
    ["AJDP-SNAPSHOT", "AJDP-FENCE"],
  Target.mk
    ``durable_replay_is_idempotent
    "durable-replay-idempotency"
    ["AJDP-REPLAY", "AJDP-IDEMPOTENT"],
  Target.mk
    ``publication_command_compatibility_binds_decision
    "publication-command-decision-binding"
    ["AJDP-COMMAND", "AJDP-IDENTITY"],
  Target.mk
    ``changed_decision_rejects_publication_replay
    "conflicting-replay-rejection"
    ["AJDP-REPLAY", "AJDP-IDENTITY"]
]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryCacheRejoinAtomicJointDecisionPublication"
    "ASPProof/SearchRouteAdmissionRetryCacheRejoinAtomicJointDecisionPublication.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinAtomicJointDecisionPublication
