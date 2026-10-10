-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteParetoFrontierVectorCompleteness
import Lean

open Lean

namespace ASPProof.Audit.SearchRouteParetoFrontierVectorCompleteness

def theoremDeclaration
    (name theoremFamily type rfcClauseId : String)
    (axioms : List String := []) : Json :=
  Json.mkObj
    [ ("name", toJson name)
    , ("kind", toJson "theorem")
    , ("theoremFamily", toJson theoremFamily)
    , ("type", toJson type)
    , ("rfcClauseIds", toJson [rfcClauseId])
    , ("axioms", toJson axioms)
    , ("hasSorryAx", toJson false)
    ]

def declarations : Array Json :=
  #[ theoremDeclaration
       "vector_coverage_implies_hop_coverage"
       "coverage-strength"
       "Cost-vector coverage implies hop coverage"
       "ASP-RFC-10.05-PFVC-VECTOR-IMPLIES-HOP"
   , theoremDeclaration
       "no_worse_before_strict_dominance"
       "dominance-composition"
       "A no-worse prefix composed with strict dominance remains strict"
       "ASP-RFC-10.05-PFVC-STRICT-COMPOSE"
   , theoremDeclaration
       "vector_complete_frontier_lifts_pareto_minimality"
       "global-pareto-lift"
       "Vector-complete frontier Pareto minimality lifts to the global catalog"
       "ASP-RFC-10.05-PFVC-GLOBAL-LIFT"
   , theoremDeclaration
       "hop_coverage_does_not_imply_vector_coverage"
       "hop-only-countermodel"
       "Hop coverage does not imply componentwise cost-vector coverage"
       "ASP-RFC-10.05-PFVC-HOP-COUNTERMODEL"
       ["Quot.sound", "propext"]
   , theoremDeclaration
       "visible_route_is_frontier_pareto_minimal"
       "frontier-local-minimality"
       "The visible route is Pareto minimal in the one-route frontier"
       "ASP-RFC-10.05-PFVC-FRONTIER-LOCAL"
   , theoremDeclaration
       "visible_route_is_not_global_pareto_minimal"
       "global-dominator-countermodel"
       "An omitted equal-hop route strictly dominates the visible route"
       "ASP-RFC-10.05-PFVC-GLOBAL-COUNTERMODEL"
       ["Quot.sound", "propext"]
   , theoremDeclaration
       "hop_complete_frontier_can_hide_global_pareto_dominator"
       "global-dominator-countermodel"
       "A hop-complete frontier can hide a global Pareto dominator"
       "ASP-RFC-10.05-PFVC-GLOBAL-COUNTERMODEL"
       ["Quot.sound", "propext"]
   ]

def manifest : Json :=
  Json.mkObj
    [ ("schemaId", toJson "asp.lean-proof-audit.v1")
    , ("schemaVersion", toJson "1")
    , ("leanVersion", toJson "4.32.2")
    , ("proofPackage", toJson "ASPProof")
    , ("module",
        toJson "ASPProof.SearchRouteParetoFrontierVectorCompleteness")
    , ("sourcePath",
        toJson
          "packages/proofs/ASPProof/SearchRouteParetoFrontierVectorCompleteness.lean")
    , ("declarationCount", toJson declarations.size)
    , ("axiomFreeDeclarationCount", toJson 4)
    , ("axiomDependentDeclarationCount", toJson 3)
    , ("declarations", toJson declarations)
    , ("axiomInventory", toJson ["Quot.sound", "propext"])
    , ("hasSorryAx", toJson false)
    , ("rfc",
        toJson "01.10-searchroute-pareto-frontier-vector-completeness")
    , ("status", toJson "kernel-compiled")
    ]

end ASPProof.Audit.SearchRouteParetoFrontierVectorCompleteness
