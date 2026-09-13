-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteVersionedCertificateInvalidation
import Lean

open Lean

namespace ASPProof.Audit.SearchRouteVersionedCertificateInvalidation

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
       "corrected_root_is_invalidated"
       "dependency-closure"
       "correctedRoots root -> Invalidated dependsOn correctedRoots root"
       "ASP-RFC-10.05-VCI-ROOT"
   , theoremDeclaration
       "direct_dependent_is_invalidated"
       "dependency-closure"
       "correctedRoots root -> dependsOn artifact root -> Invalidated dependsOn correctedRoots artifact"
       "ASP-RFC-10.05-VCI-DIRECT"
   , theoremDeclaration
       "invalidation_is_dependency_closed"
       "dependency-closure"
       "Invalidated prerequisite -> dependsOn artifact prerequisite -> Invalidated artifact"
       "ASP-RFC-10.05-VCI-CLOSED"
   , theoremDeclaration
       "invalidation_is_monotone_in_roots"
       "root-monotonicity"
       "smaller subset larger -> Invalidated smaller artifact -> Invalidated larger artifact"
       "ASP-RFC-10.05-VCI-MONOTONE"
   , theoremDeclaration
       "transitive_dependence_lifts_closed_sets"
       "closure-minimality"
       "ClosedUnderDependence artifacts -> DependsTransitively artifact root -> artifacts root -> artifacts artifact"
       "ASP-RFC-10.05-VCI-MINIMAL"
   , theoremDeclaration
       "invalidation_is_minimal"
       "closure-minimality"
       "roots subset artifacts -> ClosedUnderDependence artifacts -> Invalidated artifact -> artifacts artifact"
       "ASP-RFC-10.05-VCI-MINIMAL"
   , theoremDeclaration
       "reachability_preserves_revision"
       "version-safety"
       "edge revision preservation -> transitive reachability preserves revision"
       "ASP-RFC-10.05-VCI-VERSION"
   , theoremDeclaration
       "old_completion_is_invalidated"
       "stale-certificate"
       "Invalidated ExampleDepends correctedRoots oldCompletion"
       "ASP-RFC-10.05-VCI-STALE"
   , theoremDeclaration
       "refreshed_certificate_is_not_invalidated"
       "version-safety"
       "not (Invalidated ExampleDepends correctedRoots refreshedFeasibility)"
       "ASP-RFC-10.05-VCI-REFRESHED"
       ["propext"]
   , theoremDeclaration
       "unrelated_artifact_is_not_invalidated"
       "unrelated-preservation"
       "not (Invalidated ExampleDepends correctedRoots unrelatedCapability)"
       "ASP-RFC-10.05-VCI-UNRELATED"
       ["propext"]
   ]

def manifest : Json :=
  Json.mkObj
    [ ("schemaId", toJson "asp.lean-proof-audit.v1")
    , ("schemaVersion", toJson "1")
    , ("leanVersion", toJson "4.32.2")
    , ("proofPackage", toJson "ASPProof")
    , ("module",
        toJson "ASPProof.SearchRouteVersionedCertificateInvalidation")
    , ("sourcePath",
        toJson "ASPProof/SearchRouteVersionedCertificateInvalidation.lean")
    , ("declarationCount", toJson declarations.size)
    , ("axiomFreeDeclarationCount", toJson 8)
    , ("axiomDependentDeclarationCount", toJson 2)
    , ("declarations", toJson declarations)
    , ("axiomInventory", toJson ["propext"])
    , ("hasSorryAx", toJson false)
    , ("rfc", toJson "00.59-versioned-certificate-invalidation")
    , ("status", toJson "kernel-compiled")
    ]

end ASPProof.Audit.SearchRouteVersionedCertificateInvalidation
