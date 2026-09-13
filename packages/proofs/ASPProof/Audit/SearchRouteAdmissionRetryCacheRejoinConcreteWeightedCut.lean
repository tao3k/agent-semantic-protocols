-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteAdmissionRetryCacheRejoinConcreteWeightedCut

namespace ASPProof.Audit

def writeReceipt
    (path : System.FilePath)
    (auditJson : Lean.Elab.TermElabM Lean.Json) :
    Lean.Elab.Command.CommandElabM Unit := do
  let audit ← Lean.Elab.Command.liftTermElabM auditJson
  IO.FS.writeFile path (audit.pretty ++ "\n")
  Lean.logInfo m!"wrote {path}"

end ASPProof.Audit

namespace ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinConcreteWeightedCut

open ASPProof.Audit.Core
open ASPProof.SearchRouteAdmissionRetryCacheRejoinConcreteWeightedCut

def targets : List Target := [
  Target.mk
    ``totalVotingWeight_append
    "list-weight-append-conservation"
    ["CWC-LIST", "CWC-CONSERVATION"],
  Target.mk
    ``concrete_cut_conserves_total_weight
    "cut-partition-weight-conservation"
    ["CWC-CUT", "CWC-CONSERVATION"],
  Target.mk
    ``concrete_cut_arithmetic_witnesses
    "concrete-cut-arithmetic-witnesses"
    ["CWC-CUT", "CWC-WITNESS"],
  Target.mk
    ``selected_below_threshold_covers_lower_partition
    "ordered-lower-partition"
    ["CWC-ORDER", "CWC-LOWER"],
  Target.mk
    ``selected_above_threshold_covers_upper_partition
    "ordered-upper-partition"
    ["CWC-ORDER", "CWC-UPPER"],
  Target.mk
    ``duplicate_authority_cannot_form_distinct_admission
    "duplicate-authority-rejection"
    ["CWC-IDENTITY", "CWC-DISTINCT"],
  Target.mk
    ``duplicate_report_can_inflate_total_weight
    "duplicate-weight-inflation-counterexample"
    ["CWC-COUNTEREXAMPLE", "CWC-CONSERVATION"],
  Target.mk
    ``cumulative_cut_without_value_order_is_insufficient
    "unordered-cut-counterexample"
    ["CWC-COUNTEREXAMPLE", "CWC-ORDER"]
]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryCacheRejoinConcreteWeightedCut"
    "ASPProof/SearchRouteAdmissionRetryCacheRejoinConcreteWeightedCut.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinConcreteWeightedCut
