-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteVectorCoverageIdentityFence
import Lean

open Lean

namespace ASPProof.Audit.SearchRouteVectorCoverageIdentityFence

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
       "direct_admission_is_semantically_bound"
       "semantic-binding"
       "Direct admission implies exact semantic identity binding"
       "ASP-RFC-10.05-VCIF-SEMANTIC-BOUND"
   , theoremDeclaration
       "transition_admission_is_semantically_bound"
       "semantic-binding"
       "Transition admission implies exact semantic identity binding"
       "ASP-RFC-10.05-VCIF-SEMANTIC-BOUND"
   , theoremDeclaration
       "directly_admitted_vector_coverage_lifts_global_pareto"
       "admitted-global-lift"
       "Directly admitted vector coverage lifts frontier Pareto minimality"
       "ASP-RFC-10.05-VCIF-GLOBAL-LIFT"
   , theoremDeclaration
       "transition_admitted_vector_coverage_lifts_global_pareto"
       "admitted-global-lift"
       "Transition-admitted vector coverage lifts frontier Pareto minimality"
       "ASP-RFC-10.05-VCIF-GLOBAL-LIFT"
   , theoremDeclaration
       "same_generation_does_not_imply_semantic_binding"
       "generation-nonidentity"
       "Equal graph generation does not imply semantic identity"
       "ASP-RFC-10.05-VCIF-GENERATION-NONIDENTITY"
       ["propext"]
   , theoremDeclaration
       "generation_change_does_not_imply_semantic_change"
       "generation-nonidentity"
       "Different graph generations may preserve semantic identity"
       "ASP-RFC-10.05-VCIF-GENERATION-NONIDENTITY"
   , theoremDeclaration
       "semantic_match_without_current_fence_is_not_directly_admitted"
       "direct-generation-fence"
       "Semantic identity without a current generation fence is not direct admission"
       "ASP-RFC-10.05-VCIF-DIRECT-FENCE"
       ["propext"]
   , theoremDeclaration
       "changed_policy_rejects_direct_admission"
       "policy-identity"
       "A changed route policy rejects direct coverage admission"
       "ASP-RFC-10.05-VCIF-POLICY-IDENTITY"
       ["propext"]
   , theoremDeclaration
       "authorized_same_semantic_transition_admits_coverage"
       "transition-fence"
       "An authorized same-semantic generation transition admits coverage"
       "ASP-RFC-10.05-VCIF-TRANSITION-FENCE"
       ["propext"]
   , theoremDeclaration
       "unauthorized_transition_rejects_coverage"
       "transition-fence"
       "An unauthorized generation transition rejects coverage"
       "ASP-RFC-10.05-VCIF-TRANSITION-FENCE"
       ["propext"]
   ]

def manifest : Json :=
  Json.mkObj
    [ ("schemaId", toJson "asp.lean-proof-audit.v1")
    , ("schemaVersion", toJson "1")
    , ("leanVersion", toJson "4.32.2")
    , ("proofPackage", toJson "ASPProof")
    , ("module",
        toJson "ASPProof.SearchRouteVectorCoverageIdentityFence")
    , ("sourcePath",
        toJson
          "packages/proofs/ASPProof/SearchRouteVectorCoverageIdentityFence.lean")
    , ("declarationCount", toJson declarations.size)
    , ("axiomFreeDeclarationCount", toJson 5)
    , ("axiomDependentDeclarationCount", toJson 5)
    , ("declarations", toJson declarations)
    , ("axiomInventory", toJson ["propext"])
    , ("hasSorryAx", toJson false)
    , ("rfc",
        toJson "01.11-searchroute-vector-coverage-identity-fence")
    , ("status", toJson "kernel-compiled")
    ]

end ASPProof.Audit.SearchRouteVectorCoverageIdentityFence
