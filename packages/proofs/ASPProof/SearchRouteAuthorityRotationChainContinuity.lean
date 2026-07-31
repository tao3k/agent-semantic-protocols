import ASPProof.SearchRouteCoverageTransitionChainIntegrity

namespace ASPProof.SearchRouteAuthorityRotationChainContinuity

open ASPProof.SearchRouteGraphRouterParetoCostSelection
open ASPProof.SearchRouteParetoFrontierVectorCompleteness
open ASPProof.SearchRouteVectorCoverageIdentityFence
open ASPProof.SearchRouteCertifiedRegistryEpochTransition
open ASPProof.SearchRouteCertifiedTransitionChainCompression
open ASPProof.SearchRouteCoverageTransitionChainIntegrity

/--
Generation alone cannot identify the authority or registry snapshot that issued
a coverage proof. Rotation-safe reuse binds the complete origin clock domain.
-/
structure AuthorityBoundCoverageHeader where
  header : CoverageHeader
  issuedDomain : ClockDomain
deriving DecidableEq, Repr

def OriginDomainBound
    (boundHeader : AuthorityBoundCoverageHeader) : Prop :=
  boundHeader.issuedDomain.epoch =
    boundHeader.header.issuedGeneration

structure AuthorityTransferCertificate where
  semanticIdentity : SemanticCoverageIdentity
  fromDomain : ClockDomain
  toDomain : ClockDomain
  oldAuthorityApproved : Bool
  newAuthorityAccepted : Bool
deriving DecidableEq, Repr

structure AuthorityRotationEdge where
  coverage : GenerationTransition
  registry : TransitionCertificate
  transfer : AuthorityTransferCertificate
deriving DecidableEq, Repr

def RotationDomainsAligned (edge : AuthorityRotationEdge) : Prop :=
  edge.transfer.fromDomain = edge.registry.fromDomain ∧
    edge.transfer.toDomain = edge.registry.toDomain

def CoverageUsesSuccessorAuthority
    (boundHeader : AuthorityBoundCoverageHeader)
    (edge : AuthorityRotationEdge) : Prop :=
  edge.coverage.semanticIdentity =
      boundHeader.header.semanticIdentity ∧
    edge.coverage.fromGeneration =
      edge.registry.fromDomain.epoch ∧
    edge.coverage.toGeneration =
      edge.registry.toDomain.epoch ∧
    edge.coverage.authorityDigest =
      edge.registry.toDomain.authorityDigest ∧
    edge.coverage.authorized = true

def RotationSemanticContinuous
    (boundHeader : AuthorityBoundCoverageHeader)
    (lineage : CoverageSemanticLineage)
    (edge : AuthorityRotationEdge) : Prop :=
  edge.transfer.semanticIdentity =
      boundHeader.header.semanticIdentity ∧
    lineage.semanticAt edge.registry.fromDomain =
      boundHeader.header.semanticIdentity ∧
    lineage.semanticAt edge.registry.toDomain =
      boundHeader.header.semanticIdentity

def DualAuthorizedTransfer (edge : AuthorityRotationEdge) : Prop :=
  edge.transfer.oldAuthorityApproved = true ∧
    edge.transfer.newAuthorityAccepted = true

def OldAuthorityOnlyTransfer
    (authorizedSuccessor : ClockDomain → ClockDomain → Prop)
    (edge : AuthorityRotationEdge) : Prop :=
  ValidTransition authorizedSuccessor edge.registry ∧
    RotationDomainsAligned edge ∧
    edge.transfer.oldAuthorityApproved = true

def ValidAuthorityRotationEdge
    (authorizedSuccessor : ClockDomain → ClockDomain → Prop)
    (boundHeader : AuthorityBoundCoverageHeader)
    (lineage : CoverageSemanticLineage)
    (edge : AuthorityRotationEdge) : Prop :=
  ValidTransition authorizedSuccessor edge.registry ∧
    RotationDomainsAligned edge ∧
    edge.registry.fromDomain.authorityDigest ≠
      edge.registry.toDomain.authorityDigest ∧
    CoverageUsesSuccessorAuthority boundHeader edge ∧
    RotationSemanticContinuous boundHeader lineage edge ∧
    DualAuthorizedTransfer edge

inductive ValidAuthorityRotationChain
    (authorizedSuccessor : ClockDomain → ClockDomain → Prop)
    (boundHeader : AuthorityBoundCoverageHeader)
    (lineage : CoverageSemanticLineage) :
    ClockDomain → ClockDomain → List AuthorityRotationEdge → Prop
  | nil (domain : ClockDomain) :
      ValidAuthorityRotationChain authorizedSuccessor boundHeader lineage
        domain domain []
  | cons
      {fromDomain nextDomain finalDomain : ClockDomain}
      {edge : AuthorityRotationEdge}
      {rest : List AuthorityRotationEdge}
      (fromMatches : edge.registry.fromDomain = fromDomain)
      (toMatches : edge.registry.toDomain = nextDomain)
      (valid :
        ValidAuthorityRotationEdge
          authorizedSuccessor boundHeader lineage edge)
      (tail :
        ValidAuthorityRotationChain authorizedSuccessor boundHeader lineage
          nextDomain finalDomain rest) :
      ValidAuthorityRotationChain authorizedSuccessor boundHeader lineage
        fromDomain finalDomain (edge :: rest)

def authorityRotationRegistryCertificates :
    List AuthorityRotationEdge → List TransitionCertificate :=
  List.map AuthorityRotationEdge.registry

def AuthorityRotationChainAdmitted
    (authorizedSuccessor : ClockDomain → ClockDomain → Prop)
    (context : CoverageContext)
    (boundHeader : AuthorityBoundCoverageHeader)
    (lineage : CoverageSemanticLineage)
    (endDomain : ClockDomain)
    (edges : List AuthorityRotationEdge) : Prop :=
  SemanticallyBound context boundHeader.header ∧
    OriginDomainBound boundHeader ∧
    endDomain.epoch = context.graphGeneration ∧
    endDomain.authorityDigest =
      context.transitionAuthorityDigest ∧
    lineage.semanticAt boundHeader.issuedDomain =
      boundHeader.header.semanticIdentity ∧
    lineage.semanticAt endDomain = context.semanticIdentity ∧
    boundHeader.header.issuedGeneration <
      context.graphGeneration ∧
    ValidAuthorityRotationChain
      authorizedSuccessor
      boundHeader
      lineage
      boundHeader.issuedDomain
      endDomain
      edges

theorem valid_rotation_edge_is_old_authority_signed
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {boundHeader : AuthorityBoundCoverageHeader}
    {lineage : CoverageSemanticLineage}
    {edge : AuthorityRotationEdge}
    (valid :
      ValidAuthorityRotationEdge
        authorizedSuccessor boundHeader lineage edge) :
    edge.registry.certificateAuthorityDigest =
      edge.registry.fromDomain.authorityDigest :=
  valid.1.2.1

theorem valid_rotation_edge_is_new_authority_accepted
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {boundHeader : AuthorityBoundCoverageHeader}
    {lineage : CoverageSemanticLineage}
    {edge : AuthorityRotationEdge}
    (valid :
      ValidAuthorityRotationEdge
        authorizedSuccessor boundHeader lineage edge) :
    edge.transfer.newAuthorityAccepted = true :=
  valid.2.2.2.2.2.2

theorem valid_rotation_edge_uses_successor_authority
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {boundHeader : AuthorityBoundCoverageHeader}
    {lineage : CoverageSemanticLineage}
    {edge : AuthorityRotationEdge}
    (valid :
      ValidAuthorityRotationEdge
        authorizedSuccessor boundHeader lineage edge) :
    edge.coverage.authorityDigest =
      edge.registry.toDomain.authorityDigest :=
  valid.2.2.2.1.2.2.2.1

theorem valid_rotation_edge_preserves_semantic_lineage
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {boundHeader : AuthorityBoundCoverageHeader}
    {lineage : CoverageSemanticLineage}
    {edge : AuthorityRotationEdge}
    (valid :
      ValidAuthorityRotationEdge
        authorizedSuccessor boundHeader lineage edge) :
    lineage.semanticAt edge.registry.fromDomain =
        boundHeader.header.semanticIdentity ∧
      lineage.semanticAt edge.registry.toDomain =
        boundHeader.header.semanticIdentity :=
  ⟨valid.2.2.2.2.1.2.1, valid.2.2.2.2.1.2.2⟩

theorem missing_old_authority_approval_rejects_rotation
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {boundHeader : AuthorityBoundCoverageHeader}
    {lineage : CoverageSemanticLineage}
    {edge : AuthorityRotationEdge}
    (missing : edge.transfer.oldAuthorityApproved = false) :
    ¬ ValidAuthorityRotationEdge
      authorizedSuccessor boundHeader lineage edge := by
  intro valid
  have approved : edge.transfer.oldAuthorityApproved = true :=
    valid.2.2.2.2.2.1
  have impossible : false = true := missing.symm.trans approved
  cases impossible

theorem missing_new_authority_acceptance_rejects_rotation
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {boundHeader : AuthorityBoundCoverageHeader}
    {lineage : CoverageSemanticLineage}
    {edge : AuthorityRotationEdge}
    (missing : edge.transfer.newAuthorityAccepted = false) :
    ¬ ValidAuthorityRotationEdge
      authorizedSuccessor boundHeader lineage edge := by
  intro valid
  have accepted : edge.transfer.newAuthorityAccepted = true :=
    valid.2.2.2.2.2.2
  have impossible : false = true := missing.symm.trans accepted
  cases impossible

theorem valid_authority_rotation_chain_projects_registry_chain
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {boundHeader : AuthorityBoundCoverageHeader}
    {lineage : CoverageSemanticLineage}
    {startDomain endDomain : ClockDomain}
    {edges : List AuthorityRotationEdge}
    (chain :
      ValidAuthorityRotationChain
        authorizedSuccessor
        boundHeader
        lineage
        startDomain
        endDomain
        edges) :
    ValidChain authorizedSuccessor startDomain endDomain
      (authorityRotationRegistryCertificates edges) := by
  induction chain with
  | nil domain =>
      exact ValidChain.nil domain
  | cons fromMatches toMatches valid _ inductionHypothesis =>
      exact ValidChain.cons
        fromMatches
        toMatches
        valid.1
        inductionHypothesis

theorem valid_authority_rotation_chain_append
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {boundHeader : AuthorityBoundCoverageHeader}
    {lineage : CoverageSemanticLineage}
    {startDomain middleDomain endDomain : ClockDomain}
    {left right : List AuthorityRotationEdge}
    (leftValid :
      ValidAuthorityRotationChain
        authorizedSuccessor boundHeader lineage
        startDomain middleDomain left)
    (rightValid :
      ValidAuthorityRotationChain
        authorizedSuccessor boundHeader lineage
        middleDomain endDomain right) :
    ValidAuthorityRotationChain
      authorizedSuccessor boundHeader lineage
      startDomain endDomain (left ++ right) := by
  induction leftValid with
  | nil =>
      simpa using rightValid
  | cons fromMatches toMatches valid _ inductionHypothesis =>
      exact ValidAuthorityRotationChain.cons
        fromMatches
        toMatches
        valid
        (inductionHypothesis rightValid)

theorem valid_authority_rotation_chain_exact_epoch_span
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {boundHeader : AuthorityBoundCoverageHeader}
    {lineage : CoverageSemanticLineage}
    {startDomain endDomain : ClockDomain}
    {edges : List AuthorityRotationEdge}
    (chain :
      ValidAuthorityRotationChain
        authorizedSuccessor boundHeader lineage
        startDomain endDomain edges) :
    startDomain.epoch + edges.length = endDomain.epoch := by
  have registrySpan :=
    valid_chain_edge_count_matches_epoch_span
      (valid_authority_rotation_chain_projects_registry_chain chain)
  simpa [authorityRotationRegistryCertificates] using registrySpan

theorem functional_registry_rejects_authority_rotation_fork
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    (functional : FunctionalSuccessor authorizedSuccessor)
    {boundHeader : AuthorityBoundCoverageHeader}
    {lineage : CoverageSemanticLineage}
    {leftEdge rightEdge : AuthorityRotationEdge}
    (sameOrigin :
      leftEdge.registry.fromDomain = rightEdge.registry.fromDomain)
    (leftValid :
      ValidAuthorityRotationEdge
        authorizedSuccessor boundHeader lineage leftEdge)
    (rightValid :
      ValidAuthorityRotationEdge
        authorizedSuccessor boundHeader lineage rightEdge) :
    leftEdge.registry.toDomain = rightEdge.registry.toDomain :=
  functional_registry_rejects_successor_fork
    functional sameOrigin leftValid.1 rightValid.1

theorem authority_rotation_chain_admission_binds_current_authority
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {context : CoverageContext}
    {boundHeader : AuthorityBoundCoverageHeader}
    {lineage : CoverageSemanticLineage}
    {endDomain : ClockDomain}
    {edges : List AuthorityRotationEdge}
    (admitted :
      AuthorityRotationChainAdmitted
        authorizedSuccessor
        context
        boundHeader
        lineage
        endDomain
        edges) :
    endDomain.authorityDigest =
        context.transitionAuthorityDigest ∧
      SemanticallyBound context boundHeader.header :=
  ⟨admitted.2.2.2.1, admitted.1⟩

theorem authority_rotation_chain_admitted_vector_coverage_lifts_global_pareto
    {Candidate : Type}
    (cost : Candidate → RouteCost)
    (frontier globalCatalog : Candidate → Prop)
    (frontierSubset :
      ∀ candidate,
        frontier candidate → globalCatalog candidate)
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    (context : CoverageContext)
    (certificate : CoverageCertificate cost frontier globalCatalog)
    (issuedDomain endDomain : ClockDomain)
    (lineage : CoverageSemanticLineage)
    (edges : List AuthorityRotationEdge)
    (admitted :
      AuthorityRotationChainAdmitted
        authorizedSuccessor
        context
        {
          header := certificate.header
          issuedDomain := issuedDomain
        }
        lineage
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

def rotatedDomainNine : ClockDomain :=
  {
    registrySnapshotDigest := 209
    authorityDigest := 88
    epoch := 9
  }

def alternateAuthorityDomainEight : ClockDomain :=
  {
    registrySnapshotDigest := 108
    authorityDigest := 66
    epoch := 8
  }

def authorityRotationSuccessor : ClockDomain → ClockDomain → Prop :=
  fun fromDomain toDomain =>
    fromDomain = domainEight ∧ toDomain = rotatedDomainNine

def authorityRotationRegistryTransition : TransitionCertificate :=
  {
    fromDomain := domainEight
    toDomain := rotatedDomainNine
    oldDomainCutoff := 41
    certificateAuthorityDigest := 77
    oldSnapshotArchived := true
  }

def rotatedCoverageTransition : GenerationTransition :=
  {
    semanticIdentity := baseIdentity
    fromGeneration := 8
    toGeneration := 9
    authorityDigest := 88
    authorized := true
  }

def acceptedAuthorityTransfer : AuthorityTransferCertificate :=
  {
    semanticIdentity := baseIdentity
    fromDomain := domainEight
    toDomain := rotatedDomainNine
    oldAuthorityApproved := true
    newAuthorityAccepted := true
  }

def unacceptedAuthorityTransfer : AuthorityTransferCertificate :=
  { acceptedAuthorityTransfer with newAuthorityAccepted := false }

def acceptedAuthorityRotationEdge : AuthorityRotationEdge :=
  {
    coverage := rotatedCoverageTransition
    registry := authorityRotationRegistryTransition
    transfer := acceptedAuthorityTransfer
  }

def unacceptedAuthorityRotationEdge : AuthorityRotationEdge :=
  { acceptedAuthorityRotationEdge with
      transfer := unacceptedAuthorityTransfer }

def authorityBoundCurrentHeader : AuthorityBoundCoverageHeader :=
  {
    header := currentHeader
    issuedDomain := domainEight
  }

def rotatedContext : CoverageContext :=
  {
    semanticIdentity := baseIdentity
    graphGeneration := 9
    transitionAuthorityDigest := 88
  }

theorem generation_only_header_does_not_bind_origin_authority :
    domainEight.epoch = currentHeader.issuedGeneration ∧
      alternateAuthorityDomainEight.epoch =
        currentHeader.issuedGeneration ∧
      domainEight ≠ alternateAuthorityDomainEight := by
  decide

theorem dual_approved_rotation_edge_is_valid :
    ValidAuthorityRotationEdge
      authorityRotationSuccessor
      authorityBoundCurrentHeader
      stableSemanticLineage
      acceptedAuthorityRotationEdge := by
  simp
    [ ValidAuthorityRotationEdge
    , RotationDomainsAligned
    , CoverageUsesSuccessorAuthority
    , RotationSemanticContinuous
    , DualAuthorizedTransfer
    , ValidTransition
    , authorityRotationSuccessor
    , authorityBoundCurrentHeader
    , stableSemanticLineage
    , acceptedAuthorityRotationEdge
    , acceptedAuthorityTransfer
    , rotatedCoverageTransition
    , authorityRotationRegistryTransition
    , domainEight
    , rotatedDomainNine
    , currentHeader
    , baseIdentity
    ]

theorem old_authority_only_predicate_admits_unaccepted_rotation :
    OldAuthorityOnlyTransfer
      authorityRotationSuccessor
      unacceptedAuthorityRotationEdge := by
  simp
    [ OldAuthorityOnlyTransfer
    , RotationDomainsAligned
    , ValidTransition
    , authorityRotationSuccessor
    , unacceptedAuthorityRotationEdge
    , acceptedAuthorityRotationEdge
    , unacceptedAuthorityTransfer
    , acceptedAuthorityTransfer
    , authorityRotationRegistryTransition
    , domainEight
    , rotatedDomainNine
    ]

theorem unaccepted_new_authority_rejects_rotation :
    ¬ ValidAuthorityRotationEdge
      authorityRotationSuccessor
      authorityBoundCurrentHeader
      stableSemanticLineage
      unacceptedAuthorityRotationEdge := by
  apply missing_new_authority_acceptance_rejects_rotation
  rfl

theorem accepted_single_rotation_chain_is_admitted :
    AuthorityRotationChainAdmitted
      authorityRotationSuccessor
      rotatedContext
      authorityBoundCurrentHeader
      stableSemanticLineage
      rotatedDomainNine
      [acceptedAuthorityRotationEdge] := by
  refine ⟨rfl, rfl, rfl, rfl, rfl, rfl, by decide, ?_⟩
  exact ValidAuthorityRotationChain.cons
    rfl
    rfl
    dual_approved_rotation_edge_is_valid
    (ValidAuthorityRotationChain.nil rotatedDomainNine)

end ASPProof.SearchRouteAuthorityRotationChainContinuity
