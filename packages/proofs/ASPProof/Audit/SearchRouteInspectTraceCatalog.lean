-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteInspectTraceCatalog

namespace ASPProof.Audit.SearchRouteInspectTraceCatalog

open Lean
open ASPProof.Audit.Core

def targets : List Target :=
  [
    { name := ``SearchRouteInspectTraceCatalog.graph_candidate_lex_refl
      theoremFamily := "selector-order"
      rfcClauseIds := ["ASP-RFC-10.05-STC-SELECTOR"] },
    { name := ``SearchRouteInspectTraceCatalog.graph_candidate_lex_trans
      theoremFamily := "selector-order"
      rfcClauseIds := ["ASP-RFC-10.05-STC-SELECTOR"] },
    { name := ``SearchRouteInspectTraceCatalog.preferGraphCandidate_no_worse_left
      theoremFamily := "selector-order"
      rfcClauseIds := ["ASP-RFC-10.05-STC-SELECTOR"] },
    { name := ``SearchRouteInspectTraceCatalog.preferGraphCandidate_no_worse_right
      theoremFamily := "selector-order"
      rfcClauseIds := ["ASP-RFC-10.05-STC-SELECTOR"] },
    { name := ``SearchRouteInspectTraceCatalog.preferGraphCandidate_eq_left_or_right
      theoremFamily := "selector-membership"
      rfcClauseIds := ["ASP-RFC-10.05-STC-SELECTOR"] },
    { name := ``SearchRouteInspectTraceCatalog.chooseBestGraphCandidate_none_implies_infeasible
      theoremFamily := "selector-feasibility"
      rfcClauseIds := ["ASP-RFC-10.05-STC-SELECTOR"] },
    { name := ``SearchRouteInspectTraceCatalog.chooseBestGraphCandidate_mem_and_feasible
      theoremFamily := "selector-membership"
      rfcClauseIds := ["ASP-RFC-10.05-STC-SELECTOR"] },
    { name := ``SearchRouteInspectTraceCatalog.chooseBestGraphCandidate_is_lex_optimal
      theoremFamily := "selector-local-optimality"
      rfcClauseIds := ["ASP-RFC-10.05-STC-SELECTOR"] },
    { name := ``SearchRouteInspectTraceCatalog.selected_candidate_has_trace_provenance
      theoremFamily := "catalog-provenance"
      rfcClauseIds := ["ASP-RFC-10.05-STC-REALIZATION"] },
    { name := ``SearchRouteInspectTraceCatalog.selected_catalog_candidate_is_locally_optimal
      theoremFamily := "selector-local-optimality"
      rfcClauseIds := ["ASP-RFC-10.05-STC-SELECTOR", "ASP-RFC-10.05-STC-REALIZATION"] },
    { name := ``SearchRouteInspectTraceCatalog.complete_trace_catalog_selects_global_driver
      theoremFamily := "catalog-global-optimality"
      rfcClauseIds := [
        "ASP-RFC-10.05-STC-REALIZATION",
        "ASP-RFC-10.05-STC-COMPLETE",
        "ASP-RFC-10.05-STC-GLOBAL"
      ] },
    { name := ``SearchRouteInspectTraceCatalog.incomplete_catalog_can_hide_cheaper_driver
      theoremFamily := "counterexample"
      rfcClauseIds := ["ASP-RFC-10.05-STC-INCOMPLETE"] }
  ]

def auditJson : Elab.Term.TermElabM Json :=
  proofAuditJson
    "ASPProof.SearchRouteInspectTraceCatalog"
    "packages/proofs/ASPProof/SearchRouteInspectTraceCatalog.lean"
    targets

end ASPProof.Audit.SearchRouteInspectTraceCatalog
