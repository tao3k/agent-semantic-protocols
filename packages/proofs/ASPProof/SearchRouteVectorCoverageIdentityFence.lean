-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteParetoFrontierVectorCompleteness

namespace ASPProof.SearchRouteVectorCoverageIdentityFence

open ASPProof.SearchRouteGraphRouterParetoCostSelection
open ASPProof.SearchRouteParetoFrontierVectorCompleteness

/--
Semantic identity determines whether a coverage proof still describes the same
search decision. Graph generation is deliberately excluded: it is a serving
fence, not semantic content identity.
-/
structure SemanticCoverageIdentity where
  workspaceSnapshotDigest : Nat
  queryDigest : Nat
  routePolicyDigest : Nat
  completionKeyDigest : Nat
  globalCatalogDigest : Nat
  frontierDigest : Nat
  costDimensionDigest : Nat
  deriving DecidableEq, Repr

structure CoverageContext where
  semanticIdentity : SemanticCoverageIdentity
  graphGeneration : Nat
  transitionAuthorityDigest : Nat
  deriving DecidableEq, Repr

structure CoverageHeader where
  semanticIdentity : SemanticCoverageIdentity
  issuedGeneration : Nat
  deriving DecidableEq, Repr

structure GenerationTransition where
  semanticIdentity : SemanticCoverageIdentity
  fromGeneration : Nat
  toGeneration : Nat
  authorityDigest : Nat
  authorized : Bool
  deriving DecidableEq, Repr

structure CoverageCertificate
    {Candidate : Type}
    (cost : Candidate → RouteCost)
    (frontier globalCatalog : Candidate → Prop) where
  header : CoverageHeader
  vectorCoverage : VectorCovers cost frontier globalCatalog

def SemanticallyBound
    (context : CoverageContext)
    (header : CoverageHeader) : Prop :=
  header.semanticIdentity = context.semanticIdentity

def DirectFenceCurrent
    (context : CoverageContext)
    (header : CoverageHeader) : Prop :=
  header.issuedGeneration = context.graphGeneration

def DirectlyAdmitted
    (context : CoverageContext)
    (header : CoverageHeader) : Prop :=
  SemanticallyBound context header ∧
    DirectFenceCurrent context header

def TransitionAdmitted
    (context : CoverageContext)
    (header : CoverageHeader)
    (transition : GenerationTransition) : Prop :=
  SemanticallyBound context header ∧
    transition.semanticIdentity = header.semanticIdentity ∧
    transition.fromGeneration = header.issuedGeneration ∧
    transition.toGeneration = context.graphGeneration ∧
    transition.fromGeneration < transition.toGeneration ∧
    transition.authorityDigest = context.transitionAuthorityDigest ∧
    transition.authorized = true

theorem direct_admission_is_semantically_bound
    (context : CoverageContext)
    (header : CoverageHeader)
    (admitted : DirectlyAdmitted context header) :
    SemanticallyBound context header :=
  admitted.1

theorem transition_admission_is_semantically_bound
    (context : CoverageContext)
    (header : CoverageHeader)
    (transition : GenerationTransition)
    (admitted : TransitionAdmitted context header transition) :
    SemanticallyBound context header :=
  admitted.1

theorem directly_admitted_vector_coverage_lifts_global_pareto
    {Candidate : Type}
    (cost : Candidate → RouteCost)
    (frontier globalCatalog : Candidate → Prop)
    (frontierSubset :
      ∀ candidate,
        frontier candidate → globalCatalog candidate)
    (context : CoverageContext)
    (certificate :
      CoverageCertificate cost frontier globalCatalog)
    (admitted :
      DirectlyAdmitted context certificate.header)
    (chosen : Candidate)
    (frontierMinimal :
      ParetoMinimal cost frontier chosen) :
    ParetoMinimal cost globalCatalog chosen ∧
      certificate.header.semanticIdentity =
        context.semanticIdentity := by
  exact
    ⟨ vector_complete_frontier_lifts_pareto_minimality
        cost
        frontier
        globalCatalog
        frontierSubset
        certificate.vectorCoverage
        chosen
        frontierMinimal
    , admitted.1
    ⟩

theorem transition_admitted_vector_coverage_lifts_global_pareto
    {Candidate : Type}
    (cost : Candidate → RouteCost)
    (frontier globalCatalog : Candidate → Prop)
    (frontierSubset :
      ∀ candidate,
        frontier candidate → globalCatalog candidate)
    (context : CoverageContext)
    (certificate :
      CoverageCertificate cost frontier globalCatalog)
    (transition : GenerationTransition)
    (admitted :
      TransitionAdmitted context certificate.header transition)
    (chosen : Candidate)
    (frontierMinimal :
      ParetoMinimal cost frontier chosen) :
    ParetoMinimal cost globalCatalog chosen ∧
      certificate.header.semanticIdentity =
        context.semanticIdentity := by
  exact
    ⟨ vector_complete_frontier_lifts_pareto_minimality
        cost
        frontier
        globalCatalog
        frontierSubset
        certificate.vectorCoverage
        chosen
        frontierMinimal
    , admitted.1
    ⟩

def baseIdentity : SemanticCoverageIdentity :=
  {
    workspaceSnapshotDigest := 11
    queryDigest := 12
    routePolicyDigest := 13
    completionKeyDigest := 14
    globalCatalogDigest := 15
    frontierDigest := 16
    costDimensionDigest := 17
  }

def changedPolicyIdentity : SemanticCoverageIdentity :=
  { baseIdentity with routePolicyDigest := 99 }

def currentContext : CoverageContext :=
  {
    semanticIdentity := baseIdentity
    graphGeneration := 8
    transitionAuthorityDigest := 77
  }

def nextGenerationContext : CoverageContext :=
  { currentContext with graphGeneration := 9 }

def changedPolicyContext : CoverageContext :=
  {
    semanticIdentity := changedPolicyIdentity
    graphGeneration := 8
    transitionAuthorityDigest := 77
  }

def currentHeader : CoverageHeader :=
  {
    semanticIdentity := baseIdentity
    issuedGeneration := 8
  }

def sameSemanticTransition : GenerationTransition :=
  {
    semanticIdentity := baseIdentity
    fromGeneration := 8
    toGeneration := 9
    authorityDigest := 77
    authorized := true
  }

theorem same_generation_does_not_imply_semantic_binding :
    currentHeader.issuedGeneration =
        changedPolicyContext.graphGeneration ∧
      ¬ SemanticallyBound changedPolicyContext currentHeader := by
  simp
    [ SemanticallyBound
    , currentHeader
    , changedPolicyContext
    , changedPolicyIdentity
    , baseIdentity
    ]

theorem generation_change_does_not_imply_semantic_change :
    currentContext.graphGeneration ≠
        nextGenerationContext.graphGeneration ∧
      currentContext.semanticIdentity =
        nextGenerationContext.semanticIdentity := by
  decide

theorem semantic_match_without_current_fence_is_not_directly_admitted :
    SemanticallyBound nextGenerationContext currentHeader ∧
      ¬ DirectlyAdmitted nextGenerationContext currentHeader := by
  simp
    [ SemanticallyBound
    , DirectlyAdmitted
    , DirectFenceCurrent
    , nextGenerationContext
    , currentContext
    , currentHeader
    , baseIdentity
    ]

theorem changed_policy_rejects_direct_admission :
    ¬ DirectlyAdmitted changedPolicyContext currentHeader := by
  simp
    [ DirectlyAdmitted
    , SemanticallyBound
    , changedPolicyContext
    , changedPolicyIdentity
    , currentHeader
    , baseIdentity
    ]

theorem authorized_same_semantic_transition_admits_coverage :
    TransitionAdmitted
      nextGenerationContext
      currentHeader
      sameSemanticTransition := by
  simp
    [ TransitionAdmitted
    , SemanticallyBound
    , nextGenerationContext
    , currentContext
    , currentHeader
    , sameSemanticTransition
    , baseIdentity
    ]

def unauthorizedTransition : GenerationTransition :=
  { sameSemanticTransition with authorized := false }

theorem unauthorized_transition_rejects_coverage :
    ¬ TransitionAdmitted
      nextGenerationContext
      currentHeader
      unauthorizedTransition := by
  simp
    [ TransitionAdmitted
    , SemanticallyBound
    , nextGenerationContext
    , currentContext
    , currentHeader
    , unauthorizedTransition
    , sameSemanticTransition
    , baseIdentity
    ]

end ASPProof.SearchRouteVectorCoverageIdentityFence
