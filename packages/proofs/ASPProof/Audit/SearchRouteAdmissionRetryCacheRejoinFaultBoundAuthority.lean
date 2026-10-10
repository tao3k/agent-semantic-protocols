-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteAdmissionRetryCacheRejoinFaultBoundAuthority

namespace ASPProof.Audit

def writeReceipt
    (path : System.FilePath)
    (auditJson : Lean.Elab.TermElabM Lean.Json) :
    Lean.Elab.Command.CommandElabM Unit := do
  let audit ← Lean.Elab.Command.liftTermElabM auditJson
  IO.FS.writeFile path (audit.pretty ++ "\n")
  Lean.logInfo m!"wrote {path}"

end ASPProof.Audit

namespace ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinFaultBoundAuthority

open ASPProof.Audit.Core
open ASPProof.SearchRouteAdmissionRetryCacheRejoinFaultBoundAuthority

def targets : List Target := [
  Target.mk
    ``valid_snapshot_bounds_actual_fault_weight
    "authorized-attestation-soundness"
    ["FBAS-AUTHORITY", "FBAS-SOUND"],
  Target.mk
    ``valid_snapshot_matches_membership_and_epoch
    "membership-epoch-scope"
    ["FBAS-SCOPE", "FBAS-EPOCH"],
  Target.mk
    ``valid_snapshot_is_current_at_evaluation_time
    "temporal-validity"
    ["FBAS-TIME"],
  Target.mk
    ``authorized_current_bound_forces_actual_honest_overlap
    "validated-bound-honest-overlap"
    ["FBAS-COMPOSE", "FBAS-SOUND"],
  Target.mk
    ``unauthorized_low_bound_can_pass_arithmetic_but_not_admission
    "unauthorized-low-bound-counterexample"
    ["FBAS-REJECT", "FBAS-AUTHORITY"],
  Target.mk
    ``stale_low_bound_can_pass_arithmetic_but_not_admission
    "stale-low-bound-counterexample"
    ["FBAS-REJECT", "FBAS-TIME"]
]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryCacheRejoinFaultBoundAuthority"
    "ASPProof/SearchRouteAdmissionRetryCacheRejoinFaultBoundAuthority.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinFaultBoundAuthority
