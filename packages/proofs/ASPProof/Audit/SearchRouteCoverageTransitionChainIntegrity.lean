-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteCoverageTransitionChainIntegrity
import Lean

open Lean

namespace ASPProof.Audit.SearchRouteCoverageTransitionChainIntegrity

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
       "valid_coverage_chain_projects_registry_chain"
       "registry-chain-bridge"
       "A valid coverage chain projects to a valid registry transition chain"
       "ASP-RFC-10.05-CTCI-REGISTRY-BRIDGE"
   , theoremDeclaration
       "valid_coverage_chain_append"
       "chain-composition"
       "Connected valid coverage-chain fragments compose by append"
       "ASP-RFC-10.05-CTCI-CHAIN-COMPOSITION"
   , theoremDeclaration
       "valid_coverage_chain_covers_every_edge"
       "intermediate-semantic-integrity"
       "Every edge in a valid coverage chain satisfies the full edge predicate"
       "ASP-RFC-10.05-CTCI-INTERMEDIATE-SEMANTIC"
   , theoremDeclaration
       "valid_coverage_chain_exact_epoch_span"
       "successor-continuity"
       "Coverage-chain edge count equals the certified registry epoch span"
       "ASP-RFC-10.05-CTCI-SUCCESSOR-CONTINUITY"
       ["propext"]
   , theoremDeclaration
       "single_coverage_edge_cannot_skip_epoch"
       "successor-continuity"
       "One certified coverage edge advances exactly one registry epoch"
       "ASP-RFC-10.05-CTCI-SUCCESSOR-CONTINUITY"
       ["propext"]
   , theoremDeclaration
       "valid_coverage_edge_preserves_both_endpoint_semantics"
       "intermediate-semantic-integrity"
       "Every valid edge binds both registry endpoints to the header identity"
       "ASP-RFC-10.05-CTCI-INTERMEDIATE-SEMANTIC"
   , theoremDeclaration
       "functional_lineage_rejects_coverage_successor_fork"
       "fork-prevention"
       "A functional registry lineage rejects distinct coverage successors"
       "ASP-RFC-10.05-CTCI-FORK-PREVENTION"
   , theoremDeclaration
       "coverage_chain_admission_is_semantically_bound"
       "chain-admission"
       "Coverage-chain admission preserves exact endpoint semantic binding"
       "ASP-RFC-10.05-CTCI-REGISTRY-BRIDGE"
   , theoremDeclaration
       "chain_admitted_vector_coverage_lifts_global_pareto"
       "admitted-global-lift"
       "Chain-admitted vector coverage lifts frontier Pareto minimality globally"
       "ASP-RFC-10.05-CTCI-GLOBAL-LIFT"
   , theoremDeclaration
       "legacy_monotone_transition_admits_generation_skip"
       "legacy-skip-gap"
       "The legacy monotone transition predicate admits an epoch skip"
       "ASP-RFC-10.05-CTCI-SUCCESSOR-CONTINUITY"
       ["propext"]
   , theoremDeclaration
       "certified_adjacent_edge_is_valid"
       "registry-chain-bridge"
       "An aligned certified successor edge satisfies coverage-edge validity"
       "ASP-RFC-10.05-CTCI-REGISTRY-BRIDGE"
       ["propext"]
   , theoremDeclaration
       "intermediate_semantic_drift_rejects_adjacent_edge"
       "intermediate-semantic-integrity"
       "Semantic drift at an intermediate domain rejects coverage reuse"
       "ASP-RFC-10.05-CTCI-INTERMEDIATE-SEMANTIC"
       ["propext"]
   , theoremDeclaration
       "endpoint_binding_does_not_exclude_intermediate_drift"
       "endpoint-insufficiency"
       "Equal issued and current identities do not exclude intermediate drift"
       "ASP-RFC-10.05-CTCI-ENDPOINTS-INSUFFICIENT"
   ]

def manifest : Json :=
  Json.mkObj
    [ ("schemaId", toJson "asp.lean-proof-audit.v1")
    , ("schemaVersion", toJson "1")
    , ("leanVersion", toJson "4.32.2")
    , ("proofPackage", toJson "ASPProof")
    , ("module",
        toJson "ASPProof.SearchRouteCoverageTransitionChainIntegrity")
    , ("sourcePath",
        toJson
          "packages/proofs/ASPProof/SearchRouteCoverageTransitionChainIntegrity.lean")
    , ("declarationCount", toJson declarations.size)
    , ("axiomFreeDeclarationCount", toJson 8)
    , ("axiomDependentDeclarationCount", toJson 5)
    , ("declarations", toJson declarations)
    , ("axiomInventory", toJson ["propext"])
    , ("hasSorryAx", toJson false)
    , ("rfc",
        toJson "01.13-searchroute-coverage-transition-chain-integrity")
    , ("status", toJson "kernel-compiled")
    ]

end ASPProof.Audit.SearchRouteCoverageTransitionChainIntegrity
