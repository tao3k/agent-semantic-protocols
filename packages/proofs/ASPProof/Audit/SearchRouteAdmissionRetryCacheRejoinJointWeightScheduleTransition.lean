-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteAdmissionRetryCacheRejoinJointWeightScheduleTransition

namespace ASPProof.Audit

def writeReceipt
    (path : System.FilePath)
    (auditJson : Lean.Elab.TermElabM Lean.Json) :
    Lean.Elab.Command.CommandElabM Unit := do
  let audit ← Lean.Elab.Command.liftTermElabM auditJson
  IO.FS.writeFile path (audit.pretty ++ "\n")
  Lean.logInfo m!"wrote {path}"

end ASPProof.Audit

namespace ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinJointWeightScheduleTransition

open ASPProof.Audit.Core
open ASPProof.SearchRouteAdmissionRetryCacheRejoinJointWeightScheduleTransition

def targets : List Target := [
  Target.mk
    ``valid_phase_progression
    "old-joint-new-phase-progression"
    ["JWST-PHASE", "JWST-MONOTONE"],
  Target.mk
    ``new_only_cannot_regress_to_old_only
    "phase-regression-rejection"
    ["JWST-PHASE", "JWST-REGRESSION"],
  Target.mk
    ``old_only_rejects_new_schedule
    "old-only-schedule-authorization"
    ["JWST-AUTHORIZATION", "JWST-OLD"],
  Target.mk
    ``new_only_rejects_old_schedule
    "new-only-schedule-authorization"
    ["JWST-AUTHORIZATION", "JWST-NEW"],
  Target.mk
    ``joint_authorizes_both_schedules
    "joint-dual-schedule-authorization"
    ["JWST-AUTHORIZATION", "JWST-JOINT"],
  Target.mk
    ``receipt_compatibility_binds_weight_schedule
    "receipt-schedule-binding"
    ["JWST-RECEIPT", "JWST-SCHEDULE"],
  Target.mk
    ``changed_schedule_invalidates_receipt_compatibility
    "schedule-change-cache-invalidation"
    ["JWST-CACHE", "JWST-SCHEDULE"],
  Target.mk
    ``same_selected_value_does_not_imply_compatibility
    "same-value-compatibility-counterexample"
    ["JWST-COUNTEREXAMPLE", "JWST-RECEIPT"],
  Target.mk
    ``joint_pair_remains_two_receipts_when_selected_values_match
    "joint-separate-receipts"
    ["JWST-JOINT", "JWST-RECEIPT"],
  Target.mk
    ``one_materialized_weight_cannot_realize_two_different_schedule_weights
    "weight-reinterpretation-rejection"
    ["JWST-MATERIALIZATION", "JWST-SCHEDULE"]
]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryCacheRejoinJointWeightScheduleTransition"
    "ASPProof/SearchRouteAdmissionRetryCacheRejoinJointWeightScheduleTransition.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinJointWeightScheduleTransition
