-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteSparseFrontierCertificate

namespace ASPProof.Audit.SearchRouteSparseFrontierCertificate

open Lean
open ASPProof.Audit.Core

def targets : List Target :=
  [
    { name := ``_root_.SearchRouteSparseFrontierCertificate.certified_frontier_selection_is_global
      theoremFamily := "frontier-global-optimality"
      rfcClauseIds := [
        "ASP-RFC-10.05-SFC-COVERAGE",
        "ASP-RFC-10.05-SFC-FRONTIER",
        "ASP-RFC-10.05-SFC-GLOBAL"
      ] },
    { name := ``_root_.SearchRouteSparseFrontierCertificate.certified_frontiers_imply_catalog_distance_completeness
      theoremFamily := "frontier-to-distance-completeness"
      rfcClauseIds := [
        "ASP-RFC-10.05-SFC-DISTANCE",
        "ASP-RFC-10.05-SFC-FRONTIER"
      ] },
    { name := ``_root_.SearchRouteSparseFrontierCertificate.distance_complete_sparse_selection_is_global
      theoremFamily := "distance-completeness-to-global-optimality"
      rfcClauseIds := [
        "ASP-RFC-10.05-SFC-DISTANCE",
        "ASP-RFC-10.05-SFC-GLOBAL"
      ] },
    { name := ``_root_.SearchRouteSparseFrontierCertificate.uncovered_hidden_candidate_invalidates_global_claim
      theoremFamily := "hidden-candidate-counterexample"
      rfcClauseIds := ["ASP-RFC-10.05-SFC-HIDDEN"] }
  ]

def auditJson : Elab.Term.TermElabM Json :=
  proofAuditJson
    "ASPProof.SearchRouteSparseFrontierCertificate"
    "packages/proofs/ASPProof/SearchRouteSparseFrontierCertificate.lean"
    targets

end ASPProof.Audit.SearchRouteSparseFrontierCertificate
