-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteVectorCoverageIdentityFence
import ASPProof.SearchRouteCertifiedTransitionChainCompression

namespace ASPProof.SearchRouteCoverageTransitionChainIntegrity

open ASPProof.SearchRouteGraphRouterParetoCostSelection
open ASPProof.SearchRouteParetoFrontierVectorCompleteness
open ASPProof.SearchRouteVectorCoverageIdentityFence
open ASPProof.SearchRouteCertifiedRegistryEpochTransition
open ASPProof.SearchRouteCertifiedTransitionChainCompression

/--
The semantic identity published at each certified registry clock domain.
Endpoints alone are insufficient: every intermediate domain must be checked.
-/
structure CoverageSemanticLineage where
  semanticAt : ClockDomain → SemanticCoverageIdentity

/--
A coverage edge is not self-authenticating. It carries the registry transition
certificate that establishes its exact successor domain.
-/
structure CertifiedCoverageEdge where
  coverage : GenerationTransition
  registry : TransitionCertificate
deriving DecidableEq, Repr

def CoverageEdgeAligned
    (context : CoverageContext)
    (header : CoverageHeader)
    (lineage : CoverageSemanticLineage)
    (edge : CertifiedCoverageEdge) : Prop :=
  edge.coverage.semanticIdentity = header.semanticIdentity ∧
    edge.coverage.fromGeneration = edge.registry.fromDomain.epoch ∧
    edge.coverage.toGeneration = edge.registry.toDomain.epoch ∧
    edge.coverage.authorityDigest =
      edge.registry.certificateAuthorityDigest ∧
    edge.registry.certificateAuthorityDigest =
      context.transitionAuthorityDigest ∧
    edge.registry.toDomain.authorityDigest =
      context.transitionAuthorityDigest ∧
    lineage.semanticAt edge.registry.fromDomain =
      header.semanticIdentity ∧
    lineage.semanticAt edge.registry.toDomain =
      header.semanticIdentity ∧
    edge.coverage.authorized = true

def ValidCoverageEdge
    (authorizedSuccessor : ClockDomain → ClockDomain → Prop)
    (context : CoverageContext)
    (header : CoverageHeader)
    (lineage : CoverageSemanticLineage)
    (edge : CertifiedCoverageEdge) : Prop :=
  ValidTransition authorizedSuccessor edge.registry ∧
    CoverageEdgeAligned context header lineage edge

inductive ValidCoverageChain
    (authorizedSuccessor : ClockDomain → ClockDomain → Prop)
    (context : CoverageContext)
    (header : CoverageHeader)
    (lineage : CoverageSemanticLineage) :
    ClockDomain → ClockDomain → List CertifiedCoverageEdge → Prop
  | nil (domain : ClockDomain) :
      ValidCoverageChain authorizedSuccessor context header lineage
        domain domain []
  | cons
      {fromDomain nextDomain finalDomain : ClockDomain}
      {edge : CertifiedCoverageEdge}
      {rest : List CertifiedCoverageEdge}
      (fromMatches : edge.registry.fromDomain = fromDomain)
      (toMatches : edge.registry.toDomain = nextDomain)
      (valid :
        ValidCoverageEdge
          authorizedSuccessor context header lineage edge)
      (tail :
        ValidCoverageChain authorizedSuccessor context header lineage
          nextDomain finalDomain rest) :
      ValidCoverageChain authorizedSuccessor context header lineage
        fromDomain finalDomain (edge :: rest)

def registryCertificates :
    List CertifiedCoverageEdge → List TransitionCertificate :=
  List.map CertifiedCoverageEdge.registry

def AllCoverageEdgesValid
    (authorizedSuccessor : ClockDomain → ClockDomain → Prop)
    (context : CoverageContext)
    (header : CoverageHeader)
    (lineage : CoverageSemanticLineage) :
    List CertifiedCoverageEdge → Prop
  | [] => True
  | edge :: rest =>
      ValidCoverageEdge authorizedSuccessor context header lineage edge ∧
        AllCoverageEdgesValid
          authorizedSuccessor context header lineage rest

def CoverageChainAdmitted
    (authorizedSuccessor : ClockDomain → ClockDomain → Prop)
    (context : CoverageContext)
    (header : CoverageHeader)
    (lineage : CoverageSemanticLineage)
    (startDomain endDomain : ClockDomain)
    (edges : List CertifiedCoverageEdge) : Prop :=
  SemanticallyBound context header ∧
    startDomain.epoch = header.issuedGeneration ∧
    endDomain.epoch = context.graphGeneration ∧
    lineage.semanticAt startDomain = header.semanticIdentity ∧
    lineage.semanticAt endDomain = context.semanticIdentity ∧
    header.issuedGeneration < context.graphGeneration ∧
    ValidCoverageChain authorizedSuccessor context header lineage
      startDomain endDomain edges

theorem valid_coverage_chain_projects_registry_chain
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {context : CoverageContext}
    {header : CoverageHeader}
    {lineage : CoverageSemanticLineage}
    {startDomain endDomain : ClockDomain}
    {edges : List CertifiedCoverageEdge}
    (chain :
      ValidCoverageChain authorizedSuccessor context header lineage
        startDomain endDomain edges) :
    ValidChain authorizedSuccessor startDomain endDomain
      (registryCertificates edges) := by
  induction chain with
  | nil domain =>
      exact ValidChain.nil domain
  | cons fromMatches toMatches valid _ inductionHypothesis =>
      exact ValidChain.cons
        fromMatches
        toMatches
        valid.1
        inductionHypothesis

theorem valid_coverage_chain_append
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {context : CoverageContext}
    {header : CoverageHeader}
    {lineage : CoverageSemanticLineage}
    {startDomain middleDomain endDomain : ClockDomain}
    {left right : List CertifiedCoverageEdge}
    (leftValid :
      ValidCoverageChain authorizedSuccessor context header lineage
        startDomain middleDomain left)
    (rightValid :
      ValidCoverageChain authorizedSuccessor context header lineage
        middleDomain endDomain right) :
    ValidCoverageChain authorizedSuccessor context header lineage
      startDomain endDomain (left ++ right) := by
  induction leftValid with
  | nil =>
      simpa using rightValid
  | cons fromMatches toMatches valid _ inductionHypothesis =>
      exact ValidCoverageChain.cons
        fromMatches
        toMatches
        valid
        (inductionHypothesis rightValid)

theorem valid_coverage_chain_covers_every_edge
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {context : CoverageContext}
    {header : CoverageHeader}
    {lineage : CoverageSemanticLineage}
    {startDomain endDomain : ClockDomain}
    {edges : List CertifiedCoverageEdge}
    (chain :
      ValidCoverageChain authorizedSuccessor context header lineage
        startDomain endDomain edges) :
    AllCoverageEdgesValid authorizedSuccessor context header lineage edges := by
  induction chain with
  | nil =>
      trivial
  | cons _ _ valid _ inductionHypothesis =>
      exact ⟨valid, inductionHypothesis⟩

theorem valid_coverage_chain_exact_epoch_span
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {context : CoverageContext}
    {header : CoverageHeader}
    {lineage : CoverageSemanticLineage}
    {startDomain endDomain : ClockDomain}
    {edges : List CertifiedCoverageEdge}
    (chain :
      ValidCoverageChain authorizedSuccessor context header lineage
        startDomain endDomain edges) :
    startDomain.epoch + edges.length = endDomain.epoch := by
  have registrySpan :=
    valid_chain_edge_count_matches_epoch_span
      (valid_coverage_chain_projects_registry_chain chain)
  simpa [registryCertificates] using registrySpan

theorem single_coverage_edge_cannot_skip_epoch
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {context : CoverageContext}
    {header : CoverageHeader}
    {lineage : CoverageSemanticLineage}
    {startDomain endDomain : ClockDomain}
    {edge : CertifiedCoverageEdge}
    (chain :
      ValidCoverageChain authorizedSuccessor context header lineage
        startDomain endDomain [edge]) :
    startDomain.epoch + 1 = endDomain.epoch := by
  simpa using valid_coverage_chain_exact_epoch_span chain

theorem valid_coverage_edge_preserves_both_endpoint_semantics
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {context : CoverageContext}
    {header : CoverageHeader}
    {lineage : CoverageSemanticLineage}
    {edge : CertifiedCoverageEdge}
    (valid :
      ValidCoverageEdge
        authorizedSuccessor context header lineage edge) :
    lineage.semanticAt edge.registry.fromDomain =
        header.semanticIdentity ∧
      lineage.semanticAt edge.registry.toDomain =
        header.semanticIdentity :=
  ⟨valid.2.2.2.2.2.2.2.1, valid.2.2.2.2.2.2.2.2.1⟩

theorem functional_lineage_rejects_coverage_successor_fork
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    (functional : FunctionalSuccessor authorizedSuccessor)
    {context : CoverageContext}
    {header : CoverageHeader}
    {lineage : CoverageSemanticLineage}
    {leftEdge rightEdge : CertifiedCoverageEdge}
    (sameOrigin :
      leftEdge.registry.fromDomain = rightEdge.registry.fromDomain)
    (leftValid :
      ValidCoverageEdge
        authorizedSuccessor context header lineage leftEdge)
    (rightValid :
      ValidCoverageEdge
        authorizedSuccessor context header lineage rightEdge) :
    leftEdge.registry.toDomain = rightEdge.registry.toDomain :=
  functional_registry_rejects_successor_fork
    functional sameOrigin leftValid.1 rightValid.1

theorem coverage_chain_admission_is_semantically_bound
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {context : CoverageContext}
    {header : CoverageHeader}
    {lineage : CoverageSemanticLineage}
    {startDomain endDomain : ClockDomain}
    {edges : List CertifiedCoverageEdge}
    (admitted :
      CoverageChainAdmitted authorizedSuccessor context header lineage
        startDomain endDomain edges) :
    SemanticallyBound context header :=
  admitted.1

theorem chain_admitted_vector_coverage_lifts_global_pareto
    {Candidate : Type}
    (cost : Candidate → RouteCost)
    (frontier globalCatalog : Candidate → Prop)
    (frontierSubset :
      ∀ candidate,
        frontier candidate → globalCatalog candidate)
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    (context : CoverageContext)
    (certificate : CoverageCertificate cost frontier globalCatalog)
    (lineage : CoverageSemanticLineage)
    (startDomain endDomain : ClockDomain)
    (edges : List CertifiedCoverageEdge)
    (admitted :
      CoverageChainAdmitted
        authorizedSuccessor
        context
        certificate.header
        lineage
        startDomain
        endDomain
        edges)
    (chosen : Candidate)
    (frontierMinimal : ParetoMinimal cost frontier chosen) :
    ParetoMinimal cost globalCatalog chosen ∧
      certificate.header.semanticIdentity = context.semanticIdentity := by
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

def generationTenContext : CoverageContext :=
  { currentContext with graphGeneration := 10 }

def skippingCoverageTransition : GenerationTransition :=
  { sameSemanticTransition with toGeneration := 10 }

theorem legacy_monotone_transition_admits_generation_skip :
    TransitionAdmitted
      generationTenContext
      currentHeader
      skippingCoverageTransition := by
  simp
    [ TransitionAdmitted
    , SemanticallyBound
    , generationTenContext
    , currentContext
    , currentHeader
    , skippingCoverageTransition
    , sameSemanticTransition
    , baseIdentity
    ]

def domainEight : ClockDomain :=
  {
    registrySnapshotDigest := 108
    authorityDigest := 77
    epoch := 8
  }

def domainNine : ClockDomain :=
  {
    registrySnapshotDigest := 109
    authorityDigest := 77
    epoch := 9
  }

def domainTen : ClockDomain :=
  {
    registrySnapshotDigest := 110
    authorityDigest := 77
    epoch := 10
  }

def stableSemanticLineage : CoverageSemanticLineage :=
  { semanticAt := fun _ => baseIdentity }

def driftAtNineLineage : CoverageSemanticLineage :=
  {
    semanticAt := fun domain =>
      if domain.epoch = 9 then changedPolicyIdentity else baseIdentity
  }

def successorEightNine : ClockDomain → ClockDomain → Prop :=
  fun fromDomain toDomain =>
    fromDomain = domainEight ∧ toDomain = domainNine

def transitionEightNine : TransitionCertificate :=
  {
    fromDomain := domainEight
    toDomain := domainNine
    oldDomainCutoff := 40
    certificateAuthorityDigest := 77
    oldSnapshotArchived := true
  }

def coverageEightNine : GenerationTransition :=
  {
    semanticIdentity := baseIdentity
    fromGeneration := 8
    toGeneration := 9
    authorityDigest := 77
    authorized := true
  }

def certifiedCoverageEightNine : CertifiedCoverageEdge :=
  {
    coverage := coverageEightNine
    registry := transitionEightNine
  }

private theorem changed_policy_identity_is_not_base :
    changedPolicyIdentity ≠ baseIdentity := by
  intro identitiesEqual
  have policiesEqual :=
    congrArg SemanticCoverageIdentity.routePolicyDigest identitiesEqual
  change (99 : Nat) = 13 at policiesEqual
  exact (by decide : (99 : Nat) ≠ 13) policiesEqual

theorem certified_adjacent_edge_is_valid :
    ValidCoverageEdge
      successorEightNine
      nextGenerationContext
      currentHeader
      stableSemanticLineage
      certifiedCoverageEightNine := by
  simp
    [ ValidCoverageEdge
    , CoverageEdgeAligned
    , ValidTransition
    , successorEightNine
    , nextGenerationContext
    , currentContext
    , currentHeader
    , stableSemanticLineage
    , certifiedCoverageEightNine
    , coverageEightNine
    , transitionEightNine
    , domainEight
    , domainNine
    , baseIdentity
    ]

theorem intermediate_semantic_drift_rejects_adjacent_edge :
    ¬ ValidCoverageEdge
      successorEightNine
      nextGenerationContext
      currentHeader
      driftAtNineLineage
      certifiedCoverageEightNine := by
  intro valid
  have endpointSemantics :=
    valid_coverage_edge_preserves_both_endpoint_semantics valid
  have identitiesEqual : changedPolicyIdentity = baseIdentity := by
    simpa
      [ driftAtNineLineage
      , certifiedCoverageEightNine
      , transitionEightNine
      , domainNine
      , currentHeader
      ] using endpointSemantics.2
  exact changed_policy_identity_is_not_base identitiesEqual

theorem endpoint_binding_does_not_exclude_intermediate_drift :
    SemanticallyBound generationTenContext currentHeader ∧
      driftAtNineLineage.semanticAt domainEight =
        currentHeader.semanticIdentity ∧
      driftAtNineLineage.semanticAt domainTen =
        generationTenContext.semanticIdentity ∧
      driftAtNineLineage.semanticAt domainNine ≠
        currentHeader.semanticIdentity := by
  refine ⟨?_, ?_, ?_, ?_⟩
  · rfl
  · rfl
  · rfl
  · intro identitiesEqual
    change changedPolicyIdentity = baseIdentity at identitiesEqual
    exact changed_policy_identity_is_not_base identitiesEqual

end ASPProof.SearchRouteCoverageTransitionChainIntegrity
