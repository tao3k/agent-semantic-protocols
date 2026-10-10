-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteAdmissionRetryCacheRejoinBoundedPublicationRecoveryLease

namespace ASPProof.Audit

def writeReceipt
    (path : System.FilePath)
    (auditJson : Lean.Elab.TermElabM Lean.Json) :
    Lean.Elab.Command.CommandElabM Unit := do
  let audit ← Lean.Elab.Command.liftTermElabM auditJson
  IO.FS.writeFile path (audit.pretty ++ "\n")
  Lean.logInfo m!"wrote {path}"

end ASPProof.Audit

namespace ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinBoundedPublicationRecoveryLease

open ASPProof.Audit.Core
open ASPProof.SearchRouteAdmissionRetryCacheRejoinBoundedPublicationRecoveryLease

def targets : List Target := [
  Target.mk
    ``active_and_expired_are_disjoint
    "active-expired-boundary"
    ["BPRL-TIME", "BPRL-LEASE"],
  Target.mk
    ``maximum_duration_bounds_active_ownership
    "bounded-active-ownership"
    ["BPRL-BOUND", "BPRL-LIVENESS"],
  Target.mk
    ``active_lease_blocks_takeover
    "active-holder-exclusion"
    ["BPRL-ACTIVE", "BPRL-TAKEOVER"],
  Target.mk
    ``expired_lease_with_exact_fences_authorizes_takeover
    "expired-exact-fence-takeover"
    ["BPRL-EXPIRED", "BPRL-TAKEOVER"],
  Target.mk
    ``stale_lease_epoch_blocks_takeover
    "stale-epoch-takeover-rejection"
    ["BPRL-EPOCH", "BPRL-FENCE"],
  Target.mk
    ``stale_publication_revision_blocks_takeover
    "stale-revision-takeover-rejection"
    ["BPRL-REVISION", "BPRL-FENCE"],
  Target.mk
    ``changed_verification_snapshot_blocks_takeover
    "snapshot-takeover-rejection"
    ["BPRL-SNAPSHOT", "BPRL-FENCE"],
  Target.mk
    ``takeover_increments_epoch_and_revision
    "takeover-generation-advance"
    ["BPRL-EPOCH", "BPRL-REVISION"],
  Target.mk
    ``old_lease_epoch_is_fenced_after_takeover
    "old-holder-post-takeover-fence"
    ["BPRL-OLD-HOLDER", "BPRL-FENCE"],
  Target.mk
    ``changed_snapshot_blocks_active_holder_command
    "active-command-snapshot-fence"
    ["BPRL-ACTIVE", "BPRL-SNAPSHOT"],
  Target.mk
    ``recovery_command_compatibility_binds_lease_epoch
    "recovery-command-epoch-binding"
    ["BPRL-COMMAND", "BPRL-IDENTITY"],
  Target.mk
    ``changed_lease_epoch_rejects_recovery_replay
    "cross-epoch-replay-rejection"
    ["BPRL-REPLAY", "BPRL-EPOCH"]
]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryCacheRejoinBoundedPublicationRecoveryLease"
    "ASPProof/SearchRouteAdmissionRetryCacheRejoinBoundedPublicationRecoveryLease.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinBoundedPublicationRecoveryLease
