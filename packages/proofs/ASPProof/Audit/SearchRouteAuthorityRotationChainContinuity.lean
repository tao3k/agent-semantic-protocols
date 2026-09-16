-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteAuthorityRotationChainContinuity
import Lean

open Lean

namespace ASPProof.Audit.SearchRouteAuthorityRotationChainContinuity

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
       "valid_rotation_edge_is_old_authority_signed"
       "signature-role-separation"
       "A valid registry edge is signed by its source-domain authority"
       "ASP-RFC-10.05-ARCC-SIGNATURE-ROLES"
   , theoremDeclaration
       "valid_rotation_edge_is_new_authority_accepted"
       "dual-authorization"
       "A valid rotation edge is explicitly accepted by the new authority"
       "ASP-RFC-10.05-ARCC-DUAL-AUTHORIZATION"
   , theoremDeclaration
       "valid_rotation_edge_uses_successor_authority"
       "signature-role-separation"
       "The coverage transition is served under the successor authority"
       "ASP-RFC-10.05-ARCC-SIGNATURE-ROLES"
   , theoremDeclaration
       "valid_rotation_edge_preserves_semantic_lineage"
       "semantic-continuity"
       "Both authority-rotation domains preserve the coverage semantic identity"
       "ASP-RFC-10.05-ARCC-SEMANTIC-CONTINUITY"
   , theoremDeclaration
       "missing_old_authority_approval_rejects_rotation"
       "dual-authorization"
       "Missing old-authority approval rejects rotation reuse"
       "ASP-RFC-10.05-ARCC-ONE-SIDED-REJECTION"
   , theoremDeclaration
       "missing_new_authority_acceptance_rejects_rotation"
       "dual-authorization"
       "Missing new-authority acceptance rejects rotation reuse"
       "ASP-RFC-10.05-ARCC-ONE-SIDED-REJECTION"
   , theoremDeclaration
       "valid_authority_rotation_chain_projects_registry_chain"
       "registry-chain-bridge"
       "A valid authority-rotation chain projects to the registry trust chain"
       "ASP-RFC-10.05-ARCC-CHAIN-CONTINUITY"
   , theoremDeclaration
       "valid_authority_rotation_chain_append"
       "chain-composition"
       "Connected authority-rotation chain fragments compose by append"
       "ASP-RFC-10.05-ARCC-CHAIN-CONTINUITY"
   , theoremDeclaration
       "valid_authority_rotation_chain_exact_epoch_span"
       "successor-continuity"
       "Authority-rotation edge count equals the registry epoch span"
       "ASP-RFC-10.05-ARCC-CHAIN-CONTINUITY"
       ["propext"]
   , theoremDeclaration
       "functional_registry_rejects_authority_rotation_fork"
       "fork-prevention"
       "A functional registry lineage rejects authority-rotation successor forks"
       "ASP-RFC-10.05-ARCC-FORK-PREVENTION"
   , theoremDeclaration
       "authority_rotation_chain_admission_binds_current_authority"
       "endpoint-admission"
       "Rotation-chain admission binds the final domain to the current authority"
       "ASP-RFC-10.05-ARCC-ENDPOINT-ADMISSION"
   , theoremDeclaration
       "authority_rotation_chain_admitted_vector_coverage_lifts_global_pareto"
       "admitted-global-lift"
       "Rotation-chain-admitted coverage lifts frontier Pareto minimality globally"
       "ASP-RFC-10.05-ARCC-GLOBAL-LIFT"
   , theoremDeclaration
       "generation_only_header_does_not_bind_origin_authority"
       "origin-domain-binding"
       "One generation can identify distinct origin authorities"
       "ASP-RFC-10.05-ARCC-ORIGIN-DOMAIN"
   , theoremDeclaration
       "dual_approved_rotation_edge_is_valid"
       "dual-authorization"
       "A fully aligned dual-approved authority rotation edge is valid"
       "ASP-RFC-10.05-ARCC-DUAL-AUTHORIZATION"
       ["propext"]
   , theoremDeclaration
       "old_authority_only_predicate_admits_unaccepted_rotation"
       "one-sided-gap"
       "An old-authority-only predicate admits an unaccepted successor"
       "ASP-RFC-10.05-ARCC-ONE-SIDED-REJECTION"
       ["propext"]
   , theoremDeclaration
       "unaccepted_new_authority_rejects_rotation"
       "dual-authorization"
       "The full predicate rejects a transfer not accepted by the new authority"
       "ASP-RFC-10.05-ARCC-ONE-SIDED-REJECTION"
   , theoremDeclaration
       "accepted_single_rotation_chain_is_admitted"
       "endpoint-admission"
       "A dual-approved successor chain is admitted under the rotated context"
       "ASP-RFC-10.05-ARCC-ENDPOINT-ADMISSION"
       ["propext"]
   ]

def manifest : Json :=
  Json.mkObj
    [ ("schemaId", toJson "asp.lean-proof-audit.v1")
    , ("schemaVersion", toJson "1")
    , ("leanVersion", toJson "4.32.2")
    , ("proofPackage", toJson "ASPProof")
    , ("module",
        toJson "ASPProof.SearchRouteAuthorityRotationChainContinuity")
    , ("sourcePath",
        toJson
          "packages/proofs/ASPProof/SearchRouteAuthorityRotationChainContinuity.lean")
    , ("declarationCount", toJson declarations.size)
    , ("axiomFreeDeclarationCount", toJson 13)
    , ("axiomDependentDeclarationCount", toJson 4)
    , ("declarations", toJson declarations)
    , ("axiomInventory", toJson ["propext"])
    , ("hasSorryAx", toJson false)
    , ("rfc",
        toJson "01.15-searchroute-authority-rotation-chain-continuity")
    , ("status", toJson "kernel-compiled")
    ]

end ASPProof.Audit.SearchRouteAuthorityRotationChainContinuity
