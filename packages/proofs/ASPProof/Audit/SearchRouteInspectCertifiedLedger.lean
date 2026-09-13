-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteInspectCertifiedLedger

namespace ASPProof.Audit.SearchRouteInspectCertifiedLedger

open Lean
open ASPProof.Audit.Core

def targets : List Target :=
  [
    { name := ``SearchRouteInspectCertifiedLedger.certified_trace_ledger_snapshot_is_bound
      theoremFamily := "snapshot-binding"
      rfcClauseIds := ["ASP-RFC-10.05-SCL-SNAPSHOT"] },
    { name := ``SearchRouteInspectCertifiedLedger.certified_trace_ledger_visible_is_optimal
      theoremFamily := "visible-optimality"
      rfcClauseIds := ["ASP-RFC-10.05-SCL-VISIBLE"] },
    { name := ``SearchRouteInspectCertifiedLedger.certified_trace_ledger_selected_has_provenance
      theoremFamily := "trace-provenance"
      rfcClauseIds := ["ASP-RFC-10.05-SCL-VISIBLE"] },
    { name := ``SearchRouteInspectCertifiedLedger.certified_trace_ledger_selected_realizes
      theoremFamily := "trace-realization"
      rfcClauseIds := ["ASP-RFC-10.05-SCL-VISIBLE"] },
    { name := ``SearchRouteInspectCertifiedLedger.certified_trace_ledger_selects_global_driver
      theoremFamily := "ledger-global-optimality"
      rfcClauseIds := [
        "ASP-RFC-10.05-SCL-SNAPSHOT",
        "ASP-RFC-10.05-SCL-VISIBLE",
        "ASP-RFC-10.05-SCL-UNIVERSE",
        "ASP-RFC-10.05-SCL-COVERAGE",
        "ASP-RFC-10.05-SCL-CLOSURE",
        "ASP-RFC-10.05-SCL-GLOBAL"
      ] },
    { name := ``SearchRouteInspectCertifiedLedger.empty_schedule_is_complete
      theoremFamily := "scheduler-boundary"
      rfcClauseIds := ["ASP-RFC-10.05-SCL-CLOSURE"] },
    { name := ``SearchRouteInspectCertifiedLedger.schedule_closure_does_not_imply_candidate_completeness
      theoremFamily := "counterexample"
      rfcClauseIds := ["ASP-RFC-10.05-SCL-CLOSURE", "ASP-RFC-10.05-SCL-NONIMPLICATION"] }
  ]

def auditJson : Elab.Term.TermElabM Json :=
  proofAuditJson
    "ASPProof.SearchRouteInspectCertifiedLedger"
    "packages/proofs/ASPProof/SearchRouteInspectCertifiedLedger.lean"
    targets

end ASPProof.Audit.SearchRouteInspectCertifiedLedger
