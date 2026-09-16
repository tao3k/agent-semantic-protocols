-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteAuthorityRotationChainContinuity

namespace ASPProof.SearchRouteCryptographicTransferReceiptBinding

open ASPProof.SearchRouteGraphRouterParetoCostSelection
open ASPProof.SearchRouteParetoFrontierVectorCompleteness
open ASPProof.SearchRouteVectorCoverageIdentityFence
open ASPProof.SearchRouteCertifiedRegistryEpochTransition
open ASPProof.SearchRouteAuthorityRotationChainContinuity
open ASPProof.SearchRouteCoverageTransitionChainIntegrity

inductive SignatureRole
  | oldApprover
  | newAcceptor
deriving DecidableEq, Repr

structure TransferPayload where
  semanticIdentity : SemanticCoverageIdentity
  fromDomain : ClockDomain
  toDomain : ClockDomain
  replayDomainDigest : Nat
  nonce : Nat
  issuedAt : Nat
  expiresAt : Nat
deriving DecidableEq, Repr

structure SignatureEnvelope where
  role : SignatureRole
  authorityDigest : Nat
  keyEpoch : Nat
  algorithmDigest : Nat
  replayDomainDigest : Nat
  payloadDigest : Nat
  signatureDigest : Nat
deriving DecidableEq, Repr

structure CryptographicTransferReceipt where
  transfer : AuthorityTransferCertificate
  payload : TransferPayload
  payloadDigest : Nat
  oldSignature : SignatureEnvelope
  newSignature : SignatureEnvelope
deriving DecidableEq, Repr

structure TransferVerificationContext where
  replayDomainDigest : Nat
  now : Nat
  oldKeyEpoch : Nat
  newKeyEpoch : Nat
  oldAlgorithmDigest : Nat
  newAlgorithmDigest : Nat
  oldKeyRevoked : Bool
  newKeyRevoked : Bool
deriving DecidableEq, Repr

def PayloadAligned
    (context : TransferVerificationContext)
    (receipt : CryptographicTransferReceipt) : Prop :=
  receipt.payload.semanticIdentity =
      receipt.transfer.semanticIdentity ∧
    receipt.payload.fromDomain = receipt.transfer.fromDomain ∧
    receipt.payload.toDomain = receipt.transfer.toDomain ∧
    receipt.payload.replayDomainDigest =
      context.replayDomainDigest ∧
    receipt.transfer.oldAuthorityApproved = true ∧
    receipt.transfer.newAuthorityAccepted = true

def PayloadDigestBound
    (digestPayload : TransferPayload → Nat)
    (receipt : CryptographicTransferReceipt) : Prop :=
  receipt.payloadDigest = digestPayload receipt.payload

def PayloadTemporallyValid
    (context : TransferVerificationContext)
    (payload : TransferPayload) : Prop :=
  payload.issuedAt ≤ context.now ∧
    context.now < payload.expiresAt

def SignatureBound
    (verify : SignatureEnvelope → Prop)
    (role : SignatureRole)
    (authorityDigest keyEpoch algorithmDigest replayDomainDigest
      payloadDigest : Nat)
    (signature : SignatureEnvelope) : Prop :=
  signature.role = role ∧
    signature.authorityDigest = authorityDigest ∧
    signature.keyEpoch = keyEpoch ∧
    signature.algorithmDigest = algorithmDigest ∧
    signature.replayDomainDigest = replayDomainDigest ∧
    signature.payloadDigest = payloadDigest ∧
    verify signature

def ValidCryptographicTransferReceipt
    (digestPayload : TransferPayload → Nat)
    (verify : SignatureEnvelope → Prop)
    (context : TransferVerificationContext)
    (receipt : CryptographicTransferReceipt) : Prop :=
  PayloadAligned context receipt ∧
    PayloadDigestBound digestPayload receipt ∧
    PayloadTemporallyValid context receipt.payload ∧
    context.oldKeyRevoked = false ∧
    context.newKeyRevoked = false ∧
    SignatureBound
      verify
      .oldApprover
      receipt.payload.fromDomain.authorityDigest
      context.oldKeyEpoch
      context.oldAlgorithmDigest
      context.replayDomainDigest
      receipt.payloadDigest
      receipt.oldSignature ∧
    SignatureBound
      verify
      .newAcceptor
      receipt.payload.toDomain.authorityDigest
      context.newKeyEpoch
      context.newAlgorithmDigest
      context.replayDomainDigest
      receipt.payloadDigest
      receipt.newSignature

def SignaturesVerifiedOnly
    (verify : SignatureEnvelope → Prop)
    (receipt : CryptographicTransferReceipt) : Prop :=
  verify receipt.oldSignature ∧ verify receipt.newSignature

structure CryptographicAuthorityRotationEdge where
  rotation : AuthorityRotationEdge
  receipt : CryptographicTransferReceipt
deriving DecidableEq, Repr

def ValidCryptographicAuthorityRotationEdge
    (authorizedSuccessor : ClockDomain → ClockDomain → Prop)
    (digestPayload : TransferPayload → Nat)
    (verify : SignatureEnvelope → Prop)
    (context : TransferVerificationContext)
    (boundHeader : AuthorityBoundCoverageHeader)
    (lineage : CoverageSemanticLineage)
    (edge : CryptographicAuthorityRotationEdge) : Prop :=
  edge.receipt.transfer = edge.rotation.transfer ∧
    ValidAuthorityRotationEdge
      authorizedSuccessor boundHeader lineage edge.rotation ∧
    ValidCryptographicTransferReceipt
      digestPayload verify context edge.receipt

inductive ValidCryptographicAuthorityRotationChain
    (authorizedSuccessor : ClockDomain → ClockDomain → Prop)
    (digestPayload : TransferPayload → Nat)
    (verify : SignatureEnvelope → Prop)
    (context : TransferVerificationContext)
    (boundHeader : AuthorityBoundCoverageHeader)
    (lineage : CoverageSemanticLineage) :
    ClockDomain →
      ClockDomain →
      List CryptographicAuthorityRotationEdge →
      Prop
  | nil (domain : ClockDomain) :
      ValidCryptographicAuthorityRotationChain
        authorizedSuccessor digestPayload verify context
        boundHeader lineage domain domain []
  | cons
      {fromDomain nextDomain finalDomain : ClockDomain}
      {edge : CryptographicAuthorityRotationEdge}
      {rest : List CryptographicAuthorityRotationEdge}
      (fromMatches : edge.rotation.registry.fromDomain = fromDomain)
      (toMatches : edge.rotation.registry.toDomain = nextDomain)
      (valid :
        ValidCryptographicAuthorityRotationEdge
          authorizedSuccessor digestPayload verify context
          boundHeader lineage edge)
      (tail :
        ValidCryptographicAuthorityRotationChain
          authorizedSuccessor digestPayload verify context
          boundHeader lineage nextDomain finalDomain rest) :
      ValidCryptographicAuthorityRotationChain
        authorizedSuccessor digestPayload verify context
        boundHeader lineage fromDomain finalDomain (edge :: rest)

def authorityRotationEdges :
    List CryptographicAuthorityRotationEdge →
      List AuthorityRotationEdge :=
  List.map CryptographicAuthorityRotationEdge.rotation

def CryptographicAuthorityRotationChainAdmitted
    (authorizedSuccessor : ClockDomain → ClockDomain → Prop)
    (digestPayload : TransferPayload → Nat)
    (verify : SignatureEnvelope → Prop)
    (transferContext : TransferVerificationContext)
    (coverageContext : CoverageContext)
    (boundHeader : AuthorityBoundCoverageHeader)
    (lineage : CoverageSemanticLineage)
    (endDomain : ClockDomain)
    (edges : List CryptographicAuthorityRotationEdge) : Prop :=
  AuthorityRotationChainAdmitted
      authorizedSuccessor
      coverageContext
      boundHeader
      lineage
      endDomain
      (authorityRotationEdges edges) ∧
    ValidCryptographicAuthorityRotationChain
      authorizedSuccessor
      digestPayload
      verify
      transferContext
      boundHeader
      lineage
      boundHeader.issuedDomain
      endDomain
      edges

theorem valid_receipt_binds_payload
    {digestPayload : TransferPayload → Nat}
    {verify : SignatureEnvelope → Prop}
    {context : TransferVerificationContext}
    {receipt : CryptographicTransferReceipt}
    (valid :
      ValidCryptographicTransferReceipt
        digestPayload verify context receipt) :
    PayloadAligned context receipt ∧
      PayloadDigestBound digestPayload receipt :=
  ⟨valid.1, valid.2.1⟩

theorem valid_receipt_binds_replay_domain
    {digestPayload : TransferPayload → Nat}
    {verify : SignatureEnvelope → Prop}
    {context : TransferVerificationContext}
    {receipt : CryptographicTransferReceipt}
    (valid :
      ValidCryptographicTransferReceipt
        digestPayload verify context receipt) :
    receipt.payload.replayDomainDigest =
        context.replayDomainDigest ∧
      receipt.oldSignature.replayDomainDigest =
        context.replayDomainDigest ∧
      receipt.newSignature.replayDomainDigest =
        context.replayDomainDigest :=
  ⟨valid.1.2.2.2.1, valid.2.2.2.2.2.1.2.2.2.2.1,
    valid.2.2.2.2.2.2.2.2.2.2.1⟩

theorem valid_receipt_is_temporally_scoped
    {digestPayload : TransferPayload → Nat}
    {verify : SignatureEnvelope → Prop}
    {context : TransferVerificationContext}
    {receipt : CryptographicTransferReceipt}
    (valid :
      ValidCryptographicTransferReceipt
        digestPayload verify context receipt) :
    PayloadTemporallyValid context receipt.payload :=
  valid.2.2.1

theorem expired_payload_rejects_receipt
    {digestPayload : TransferPayload → Nat}
    {verify : SignatureEnvelope → Prop}
    {context : TransferVerificationContext}
    {receipt : CryptographicTransferReceipt}
    (expired : receipt.payload.expiresAt ≤ context.now) :
    ¬ ValidCryptographicTransferReceipt
      digestPayload verify context receipt := by
  intro valid
  exact Nat.not_lt_of_ge expired valid.2.2.1.2

theorem revoked_old_key_rejects_receipt
    {digestPayload : TransferPayload → Nat}
    {verify : SignatureEnvelope → Prop}
    {context : TransferVerificationContext}
    {receipt : CryptographicTransferReceipt}
    (revoked : context.oldKeyRevoked = true) :
    ¬ ValidCryptographicTransferReceipt
      digestPayload verify context receipt := by
  intro valid
  have usable : context.oldKeyRevoked = false :=
    valid.2.2.2.1
  have impossible : true = false := revoked.symm.trans usable
  cases impossible

theorem revoked_new_key_rejects_receipt
    {digestPayload : TransferPayload → Nat}
    {verify : SignatureEnvelope → Prop}
    {context : TransferVerificationContext}
    {receipt : CryptographicTransferReceipt}
    (revoked : context.newKeyRevoked = true) :
    ¬ ValidCryptographicTransferReceipt
      digestPayload verify context receipt := by
  intro valid
  have usable : context.newKeyRevoked = false :=
    valid.2.2.2.2.1
  have impossible : true = false := revoked.symm.trans usable
  cases impossible

theorem changed_replay_domain_rejects_receipt
    {digestPayload : TransferPayload → Nat}
    {verify : SignatureEnvelope → Prop}
    {context : TransferVerificationContext}
    {receipt : CryptographicTransferReceipt}
    (changed :
      receipt.payload.replayDomainDigest ≠
        context.replayDomainDigest) :
    ¬ ValidCryptographicTransferReceipt
      digestPayload verify context receipt := by
  intro valid
  exact changed valid.1.2.2.2.1

theorem old_algorithm_substitution_rejects_receipt
    {digestPayload : TransferPayload → Nat}
    {verify : SignatureEnvelope → Prop}
    {context : TransferVerificationContext}
    {receipt : CryptographicTransferReceipt}
    (changed :
      receipt.oldSignature.algorithmDigest ≠
        context.oldAlgorithmDigest) :
    ¬ ValidCryptographicTransferReceipt
      digestPayload verify context receipt := by
  intro valid
  exact changed valid.2.2.2.2.2.1.2.2.2.1

theorem new_key_epoch_substitution_rejects_receipt
    {digestPayload : TransferPayload → Nat}
    {verify : SignatureEnvelope → Prop}
    {context : TransferVerificationContext}
    {receipt : CryptographicTransferReceipt}
    (changed :
      receipt.newSignature.keyEpoch ≠ context.newKeyEpoch) :
    ¬ ValidCryptographicTransferReceipt
      digestPayload verify context receipt := by
  intro valid
  exact changed valid.2.2.2.2.2.2.2.2.1

theorem injective_payload_digest_makes_bound_payload_unique
    {digestPayload : TransferPayload → Nat}
    (injective : Function.Injective digestPayload)
    {left right : CryptographicTransferReceipt}
    (leftBound : PayloadDigestBound digestPayload left)
    (rightBound : PayloadDigestBound digestPayload right)
    (sameDigest : left.payloadDigest = right.payloadDigest) :
    left.payload = right.payload := by
  apply injective
  calc
    digestPayload left.payload = left.payloadDigest := leftBound.symm
    _ = right.payloadDigest := sameDigest
    _ = digestPayload right.payload := rightBound

theorem valid_cryptographic_rotation_edge_is_valid_rotation
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {digestPayload : TransferPayload → Nat}
    {verify : SignatureEnvelope → Prop}
    {context : TransferVerificationContext}
    {boundHeader : AuthorityBoundCoverageHeader}
    {lineage : CoverageSemanticLineage}
    {edge : CryptographicAuthorityRotationEdge}
    (valid :
      ValidCryptographicAuthorityRotationEdge
        authorizedSuccessor digestPayload verify context
        boundHeader lineage edge) :
    ValidAuthorityRotationEdge
      authorizedSuccessor boundHeader lineage edge.rotation :=
  valid.2.1

theorem valid_cryptographic_chain_projects_authority_rotation_chain
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {digestPayload : TransferPayload → Nat}
    {verify : SignatureEnvelope → Prop}
    {context : TransferVerificationContext}
    {boundHeader : AuthorityBoundCoverageHeader}
    {lineage : CoverageSemanticLineage}
    {startDomain endDomain : ClockDomain}
    {edges : List CryptographicAuthorityRotationEdge}
    (chain :
      ValidCryptographicAuthorityRotationChain
        authorizedSuccessor digestPayload verify context
        boundHeader lineage startDomain endDomain edges) :
    ValidAuthorityRotationChain
      authorizedSuccessor boundHeader lineage
      startDomain endDomain (authorityRotationEdges edges) := by
  induction chain with
  | nil domain =>
      exact ValidAuthorityRotationChain.nil domain
  | cons fromMatches toMatches valid _ inductionHypothesis =>
      exact ValidAuthorityRotationChain.cons
        fromMatches
        toMatches
        valid.2.1
        inductionHypothesis

theorem valid_cryptographic_chain_exact_epoch_span
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {digestPayload : TransferPayload → Nat}
    {verify : SignatureEnvelope → Prop}
    {context : TransferVerificationContext}
    {boundHeader : AuthorityBoundCoverageHeader}
    {lineage : CoverageSemanticLineage}
    {startDomain endDomain : ClockDomain}
    {edges : List CryptographicAuthorityRotationEdge}
    (chain :
      ValidCryptographicAuthorityRotationChain
        authorizedSuccessor digestPayload verify context
        boundHeader lineage startDomain endDomain edges) :
    startDomain.epoch + edges.length = endDomain.epoch := by
  have span :=
    valid_authority_rotation_chain_exact_epoch_span
      (valid_cryptographic_chain_projects_authority_rotation_chain chain)
  simpa [authorityRotationEdges] using span

theorem cryptographic_chain_admitted_vector_coverage_lifts_global_pareto
    {Candidate : Type}
    (cost : Candidate → RouteCost)
    (frontier globalCatalog : Candidate → Prop)
    (frontierSubset :
      ∀ candidate,
        frontier candidate → globalCatalog candidate)
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {digestPayload : TransferPayload → Nat}
    {verify : SignatureEnvelope → Prop}
    (transferContext : TransferVerificationContext)
    (coverageContext : CoverageContext)
    (certificate : CoverageCertificate cost frontier globalCatalog)
    (issuedDomain endDomain : ClockDomain)
    (lineage : CoverageSemanticLineage)
    (edges : List CryptographicAuthorityRotationEdge)
    (admitted :
      CryptographicAuthorityRotationChainAdmitted
        authorizedSuccessor
        digestPayload
        verify
        transferContext
        coverageContext
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
      certificate.header.semanticIdentity =
        coverageContext.semanticIdentity := by
  exact
    ⟨ vector_complete_frontier_lifts_pareto_minimality
        cost
        frontier
        globalCatalog
        frontierSubset
        certificate.vectorCoverage
        chosen
        frontierMinimal
    , admitted.1.1
    ⟩

def examplePayload : TransferPayload :=
  {
    semanticIdentity := baseIdentity
    fromDomain := domainEight
    toDomain := rotatedDomainNine
    replayDomainDigest := 900
    nonce := 1
    issuedAt := 5
    expiresAt := 10
  }

def collisionPayload : TransferPayload :=
  { examplePayload with nonce := 2 }

def constantPayloadDigest (_ : TransferPayload) : Nat := 501

def exampleSignatureVerify (signature : SignatureEnvelope) : Prop :=
  (signature.role = .oldApprover ∧ signature.signatureDigest = 7001) ∨
    (signature.role = .newAcceptor ∧ signature.signatureDigest = 7002)

def oldExampleSignature : SignatureEnvelope :=
  {
    role := .oldApprover
    authorityDigest := 77
    keyEpoch := 3
    algorithmDigest := 101
    replayDomainDigest := 900
    payloadDigest := 501
    signatureDigest := 7001
  }

def newExampleSignature : SignatureEnvelope :=
  {
    role := .newAcceptor
    authorityDigest := 88
    keyEpoch := 4
    algorithmDigest := 202
    replayDomainDigest := 900
    payloadDigest := 501
    signatureDigest := 7002
  }

def exampleCryptographicReceipt : CryptographicTransferReceipt :=
  {
    transfer := acceptedAuthorityTransfer
    payload := examplePayload
    payloadDigest := 501
    oldSignature := oldExampleSignature
    newSignature := newExampleSignature
  }

def exampleVerificationContext : TransferVerificationContext :=
  {
    replayDomainDigest := 900
    now := 7
    oldKeyEpoch := 3
    newKeyEpoch := 4
    oldAlgorithmDigest := 101
    newAlgorithmDigest := 202
    oldKeyRevoked := false
    newKeyRevoked := false
  }

def wrongReplayContext : TransferVerificationContext :=
  { exampleVerificationContext with replayDomainDigest := 901 }

def expiredVerificationContext : TransferVerificationContext :=
  { exampleVerificationContext with now := 12 }

def migratedAlgorithmContext : TransferVerificationContext :=
  { exampleVerificationContext with oldAlgorithmDigest := 303 }

def revokedNewKeyContext : TransferVerificationContext :=
  { exampleVerificationContext with newKeyRevoked := true }

theorem constant_digest_collision_does_not_identify_payload :
    constantPayloadDigest examplePayload =
        constantPayloadDigest collisionPayload ∧
      examplePayload ≠ collisionPayload := by
  decide

theorem example_cryptographic_receipt_is_valid :
    ValidCryptographicTransferReceipt
      constantPayloadDigest
      exampleSignatureVerify
      exampleVerificationContext
      exampleCryptographicReceipt := by
  simp
    [ ValidCryptographicTransferReceipt
    , PayloadAligned
    , PayloadDigestBound
    , PayloadTemporallyValid
    , SignatureBound
    , constantPayloadDigest
    , exampleSignatureVerify
    , exampleVerificationContext
    , exampleCryptographicReceipt
    , examplePayload
    , acceptedAuthorityTransfer
    , oldExampleSignature
    , newExampleSignature
    , domainEight
    , rotatedDomainNine
    , baseIdentity
    ]

theorem signature_verification_only_allows_cross_domain_replay :
    SignaturesVerifiedOnly
        exampleSignatureVerify
        exampleCryptographicReceipt ∧
      ¬ ValidCryptographicTransferReceipt
        constantPayloadDigest
        exampleSignatureVerify
        wrongReplayContext
        exampleCryptographicReceipt := by
  constructor
  · simp
      [ SignaturesVerifiedOnly
      , exampleSignatureVerify
      , exampleCryptographicReceipt
      , oldExampleSignature
      , newExampleSignature
      ]
  · apply changed_replay_domain_rejects_receipt
    decide

theorem expired_example_receipt_is_rejected :
    ¬ ValidCryptographicTransferReceipt
      constantPayloadDigest
      exampleSignatureVerify
      expiredVerificationContext
      exampleCryptographicReceipt := by
  apply expired_payload_rejects_receipt
  decide

theorem algorithm_migration_rejects_old_receipt :
    ¬ ValidCryptographicTransferReceipt
      constantPayloadDigest
      exampleSignatureVerify
      migratedAlgorithmContext
      exampleCryptographicReceipt := by
  apply old_algorithm_substitution_rejects_receipt
  decide

theorem revoked_new_key_rejects_example_receipt :
    ¬ ValidCryptographicTransferReceipt
      constantPayloadDigest
      exampleSignatureVerify
      revokedNewKeyContext
      exampleCryptographicReceipt := by
  apply revoked_new_key_rejects_receipt
  rfl

def exampleCryptographicRotationEdge :
    CryptographicAuthorityRotationEdge :=
  {
    rotation := acceptedAuthorityRotationEdge
    receipt := exampleCryptographicReceipt
  }

theorem example_cryptographic_rotation_edge_is_valid :
    ValidCryptographicAuthorityRotationEdge
      authorityRotationSuccessor
      constantPayloadDigest
      exampleSignatureVerify
      exampleVerificationContext
      authorityBoundCurrentHeader
      stableSemanticLineage
      exampleCryptographicRotationEdge := by
  exact
    ⟨ rfl
    , dual_approved_rotation_edge_is_valid
    , example_cryptographic_receipt_is_valid
    ⟩

end ASPProof.SearchRouteCryptographicTransferReceiptBinding
